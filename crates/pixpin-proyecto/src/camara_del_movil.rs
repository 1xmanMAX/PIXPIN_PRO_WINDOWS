//! **El movil como camara de la reunion que graba el PC** (27-oct-2026):
//! el lado del PC, sin ventanas, para probarlo de punta a punta.
//!
//! El cable es `pixpin_sincro::camara`; aqui esta lo que hace el PC con
//! el:
//!
//! - **Lleva el reloj de la grabacion** ([`Camaras::empezar`],
//!   [`Camaras::seguir_a`] con lo que dice la grabadora a cada vuelta de la
//!   barra, [`Camaras::acabar`]) y lo anuncia ([`Camaras::anuncio`]): el id,
//!   el titulo, el milisegundo en que va y si esta en pausa.
//! - **Recibe cada foto o clip** ([`Camaras::recibir`]): su minuto es el que
//!   calculo el movil (`ms`) o, si no lo trae, el de su hora con el desfase
//!   de los relojes (`pixpin_sincro::camara::ms_de_la_foto`).
//! - **Lo pone en la clase** ([`adjuntar`]) como una captura mas: una
//!   IMAGEN (o el ARCHIVO `.mp4` de un clip) con `resto.enGrabacion` (`{de,
//!   ms}`, y `duracionMs` el clip), `resto.fuente: "movil"`
//!   (`presencial::FUENTE`; la camara del equipo dice `"camara"`), en la lista de
//!   la nota (`capturas` o `clips`, la misma de la reunion presencial) y
//!   dentro de su carpeta, en su minuto.
//!   La foto entra tal cual llego: sus bytes, con su EXIF.
//!
//! **Cuando entra**: la clase se guarda al «Terminar» (la nota de voz no
//! existe antes). Lo que llega grabando espera en una cola, en disco, y
//! entra al guardarse ([`Camaras::guardada`]) con su minuto. Lo que llega
//! despues (el movil lo subio tarde, sin red) entra al momento en la clase
//! ya guardada. Las ultimas grabaciones acabadas se recuerdan un rato
//! ([`ACABADAS`]) para eso.
//!
//! **Por su hora** (28-oct-2026, [`crate::por_su_hora`]): el reloj de cada
//! grabacion se apunta en disco mientras se graba
//! ([`Camaras::apuntar_reloj`], por si el programa se cae) y entra en la
//! nota al guardarse (`resto.reloj`). Una foto que no sabe de que grabacion
//! es (se hizo sin conexion) o de una que ya no se recuerda entra por su
//! hora (`tomada_en`): en la de ahora o una acabada si cae dentro, o en la
//! guardada de cualquier chat ([`por_su_hora::buscar_por_hora`]); si no cae
//! en ninguna, se niega y el movil la deja como una foto normal. La misma
//! foto dos veces (por su `clave`, el uid de la foto en el movil) entra una.
//!
//! **Sin perder nada** (28-oct-2026): cada llegada en cola lleva al lado su
//! ficha (`<fichero>.json`: minuto, lo que dura, de quien), escrita entera
//! en el acto. Si PixPin se cierra de golpe antes de guardar la clase, la
//! memoria se pierde pero la cola no: al recuperar la grabacion
//! (`grabacion_a_medias`), [`cola_en_disco`] la vuelve a leer y
//! [`recuperar_cola`] la mete en su minuto.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_sincro::al_lienzo::Suelto;
use pixpin_sincro::camara::{self, Anuncio, Reloj};

use crate::por_su_hora::{self, RelojDeGrabacion};
use crate::{almacen, carpeta_de_paginas, clase, cuaderno, presencial};

/// Si un mensaje lo hizo la camara del movil en una grabacion
/// (`resto.fuente: "movil"`, ver [`presencial::FUENTE`]: la misma clave que
/// lo de la camara del equipo, con otro valor).
pub fn es_del_movil(m: &cuaderno::Mensaje) -> bool {
    presencial::es_del_movil(&m.resto)
}

/// Cuantas grabaciones acabadas se recuerdan para lo que llegue tarde.
pub const ACABADAS: usize = 4;
/// Lo que se deja pasar entre la grabadora y el reloj antes de corregirlo.
const TOLERANCIA_MS: i64 = 250;

/// **Donde quedo una clase guardada**: el proyecto, su nota (si entro) y su
/// carpeta (si se hizo). Con esto entra lo que llega del movil.
#[derive(Clone)]
pub struct Destino {
    pub raiz: PathBuf,
    pub proyecto: String,
    pub voz: Option<String>,
    pub carpeta: Option<String>,
    /// El codigo de este equipo para el sello (`K7Q2`).
    pub aparato: String,
    /// La hora del mensaje nuevo, sin repetir milisegundo con otro de este
    /// equipo (el id de un mensaje es su hora).
    pub hora: fn() -> i64,
}

impl std::fmt::Debug for Destino {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Destino")
            .field("proyecto", &self.proyecto)
            .field("voz", &self.voz)
            .field("carpeta", &self.carpeta)
            .finish()
    }
}

/// Una foto o un clip llegado, esperando a que la clase se guarde.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Llegada {
    pub ms: i64,
    /// Un clip: lo que dura. Una foto, 0.
    pub duracion_ms: i64,
    pub clip: bool,
    /// Donde espera (en la cola de [`Camaras`]).
    pub fichero: PathBuf,
    pub nombre: String,
    /// Del aparato que la hizo.
    pub de: String,
    pub tomada_en: i64,
    /// El uid de la foto en su aparato (la `clave` de la toma): con el no
    /// entra dos veces.
    #[serde(default)]
    pub clave: String,
}

/// Lo que acaba de llegar, para el aviso de la barra («Foto del móvil ·
/// 12:34»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recibida {
    pub ms: i64,
    pub clip: bool,
    pub de: String,
}

/// Una grabacion, en curso o acabada.
#[derive(Debug)]
struct Grabacion {
    id: String,
    titulo: String,
    reloj: Reloj,
    /// La de verdad, la de la nota, cuando se sabe.
    duracion: Option<i64>,
    cola: Vec<Llegada>,
    destino: Option<Destino>,
    recibidas: usize,
}

impl Grabacion {
    fn tope(&self, ahora: i64) -> i64 {
        self.duracion
            .unwrap_or_else(|| self.reloj.duracion(ahora))
            .max(0)
    }
}

/// **Las grabaciones de este PC que el movil puede ver**: la que esta en
/// curso y las ultimas acabadas.
#[derive(Debug)]
pub struct Camaras {
    actual: Option<Grabacion>,
    acabadas: Vec<Grabacion>,
    /// Donde espera lo que llega antes de que se guarde la clase.
    cola: PathBuf,
    /// El nombre de este equipo («Cámara para la reunión del PC …»).
    pub aparato: String,
    /// Si se anuncia y se aceptan fotos (el interruptor «📱 Cámara del
    /// móvil» de la barra).
    pub encendida: bool,
    /// Los codecs de video que sabe ver este equipo (`h264`, `hevc`).
    pub codecs: Vec<String>,
    /// El desfase medido con cada aparato (lo que va adelantado su reloj).
    desfases: BTreeMap<String, i64>,
    nuevas: Vec<Recibida>,
    /// El codigo de este equipo (`K7Q2`): el `aparato` del reloj de sus
    /// grabaciones.
    pub codigo: String,
    /// Donde buscar las grabaciones ya guardadas (la carpeta de datos) y la
    /// hora sin repetir de los mensajes nuevos: sin ello, solo las de
    /// memoria.
    pub almacen: Option<(PathBuf, fn() -> i64)>,
    /// La grabacion de otro aparato que admite lo de aqui (ver [`Ajena`]).
    ajena: Option<Ajena>,
}

/// Lo que dura una grabacion ajena sin que su aparato vuelva a decir como
/// va: pasado esto se da por perdida (el movil se fue de la red).
pub const AJENA_CADUCA_MS: i64 = 90_000;

/// **La grabacion de otro aparato** (28-oct-2026, «De la PC a la grabación
/// del móvil»): el movil graba y este equipo le puede mandar capturas de su
/// pantalla con su minuto. Lo que dijo su anuncio, su reloj pasado al de
/// aqui y cuando se oyo por ultima vez.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ajena {
    pub de_id: String,
    pub de_nombre: String,
    pub anuncio: Anuncio,
    /// Su reloj, en el de aqui.
    pub reloj: Reloj,
    /// Lo que va adelantado su reloj sobre el de aqui.
    pub desfase: i64,
    pub visto: i64,
}

impl Ajena {
    /// **Lo que lleva una captura hecha aqui en `ahora`** para su
    /// grabacion: su minuto (en su grabacion), la hora de aqui y lo que va
    /// adelantado el reloj de aqui sobre el suyo, para que la ponga por su
    /// hora si prefiere.
    pub fn foto(&self, ahora: i64, clave: &str) -> camara::FotoDeCamara {
        camara::FotoDeCamara {
            grabacion: self.anuncio.grabacion.clone(),
            ms: Some(self.reloj.ms_en(ahora)),
            tomada_en: ahora,
            reloj: -self.desfase,
            clave: clave.to_string(),
            ..Default::default()
        }
    }
}

impl Camaras {
    pub fn nuevas(cola: PathBuf, aparato: &str) -> Camaras {
        Camaras {
            actual: None,
            acabadas: Vec::new(),
            cola,
            aparato: aparato.to_string(),
            encendida: true,
            codecs: vec![camara::H264.into()],
            desfases: BTreeMap::new(),
            nuevas: Vec::new(),
            codigo: String::new(),
            almacen: None,
            ajena: None,
        }
    }

    /// **El otro dijo como va su grabacion** (`camara_remota` con su
    /// `grabacion`): se apunta con su reloj pasado al de aqui. Devuelve si
    /// es nueva (otra grabacion, o la primera): para sacar el boton.
    pub fn apuntar_ajena(
        &mut self,
        de_id: &str,
        de_nombre: &str,
        a: &Anuncio,
        desfase: i64,
        ahora: i64,
    ) -> bool {
        let nueva = self
            .ajena
            .as_ref()
            .is_none_or(|x| x.anuncio.grabacion != a.grabacion || x.de_id != de_id);
        self.ajena = Some(Ajena {
            de_id: de_id.to_string(),
            de_nombre: de_nombre.to_string(),
            anuncio: a.clone(),
            reloj: Reloj::del_anuncio(a, desfase, ahora),
            desfase,
            visto: ahora,
        });
        nueva && a.grabando()
    }

    /// **La grabacion ajena viva**: la que se oyo hace menos de
    /// [`AJENA_CADUCA_MS`] y sigue grabando. `None` si no hay o acabo.
    pub fn ajena(&self, ahora: i64) -> Option<&Ajena> {
        self.ajena
            .as_ref()
            .filter(|a| a.anuncio.grabando() && ahora - a.visto <= AJENA_CADUCA_MS)
    }

    /// **El reloj de la grabacion `g` como va en la nota** (hora de pared de
    /// este equipo), visto en `ahora`.
    fn reloj_de(&self, g: &Grabacion, ahora: i64) -> RelojDeGrabacion {
        RelojDeGrabacion {
            aparato: self.codigo.clone(),
            inicio: g.reloj.desde,
            fin: g.reloj.fin.unwrap_or(0),
            pausas: g.reloj.pausas_al_cable(),
            visto: if g.reloj.fin.is_some() { 0 } else { ahora },
        }
    }

    /// El reloj de la que esta en curso, con su id.
    pub fn reloj_en_curso(&self, ahora: i64) -> Option<(String, RelojDeGrabacion)> {
        let g = self.actual.as_ref()?;
        Some((g.id.clone(), self.reloj_de(g, ahora)))
    }

    /// **Apunta en disco el reloj de la que esta en curso** (al empezar,
    /// pausar, seguir, acabar y cada poco): si el programa se cae, lo
    /// grabado se recupera con su ventana ([`relojes_sueltos`]) y las fotos
    /// que lleguen despues aun encuentran su sitio.
    pub fn apuntar_reloj(&self, ahora: i64) -> std::io::Result<()> {
        let Some((id, r)) = self.reloj_en_curso(ahora) else {
            return Ok(());
        };
        let f = fichero_del_reloj(&self.cola, &id);
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = f.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(&r).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, &f)
    }

    /// El id de la que esta en curso.
    pub fn en_curso(&self) -> Option<&str> {
        self.actual.as_ref().map(|g| g.id.as_str())
    }

    /// Lo llegado a la que esta en curso.
    pub fn recibidas(&self) -> usize {
        self.actual.as_ref().map_or(0, |g| g.recibidas)
    }

    /// **Empieza una grabacion** con id `id`, que ya va por `ms`. La que
    /// hubiera en curso se da por acabada.
    pub fn empezar(&mut self, id: &str, titulo: &str, ahora: i64, ms: i64) {
        self.acabar(ahora);
        self.actual = Some(Grabacion {
            id: id.to_string(),
            titulo: titulo.to_string(),
            reloj: Reloj::empezar(ahora, ms),
            duracion: None,
            cola: Vec::new(),
            destino: None,
            recibidas: 0,
        });
    }

    /// Con lo que dice la grabadora a cada vuelta de la barra. Devuelve si
    /// cambio la pausa (para avisar al movil).
    pub fn seguir_a(&mut self, ahora: i64, ms: i64, pausa: bool) -> bool {
        let Some(g) = self.actual.as_mut() else {
            return false;
        };
        let antes = g.reloj.en_pausa();
        g.reloj.seguir_a(ahora, ms, pausa, TOLERANCIA_MS);
        antes != g.reloj.en_pausa()
    }

    /// **Acaba la que esta en curso.** Lo suyo queda en la cola hasta
    /// [`Camaras::guardada`].
    pub fn acabar(&mut self, ahora: i64) {
        if let Some(g) = self.actual.as_mut() {
            g.reloj.acabar(ahora);
        }
        // Con su fin, por si no llega a guardarse.
        let _ = self.apuntar_reloj(ahora);
        if let Some(g) = self.actual.take() {
            self.acabadas.insert(0, g);
            while self.acabadas.len() > ACABADAS {
                if let Some(vieja) = self.acabadas.pop() {
                    tirar_cola(&vieja);
                }
            }
        }
    }

    /// **La grabacion se tiro** sin guardarse: lo suyo tambien.
    pub fn tirar(&mut self) {
        let tirada = match self.actual.take() {
            Some(g) => Some(g),
            None => self
                .acabadas
                .iter()
                .position(|g| g.destino.is_none())
                .map(|i| self.acabadas.remove(i)),
        };
        if let Some(g) = tirada {
            tirar_cola(&g);
            olvidar_reloj(&self.cola, &g.id);
        }
    }

    /// **La clase se guardo** en `destino` y su nota dura `duracion` (si
    /// entro): lo que esperaba entra ya, en su minuto. Va a la ultima
    /// acabada sin guardar (o a la de ahora, si aun no se acabo). Devuelve
    /// cuantas entraron.
    pub fn guardada(&mut self, destino: Destino, duracion: Option<i64>, ahora: i64) -> usize {
        if self.acabadas.iter().all(|g| g.destino.is_some()) {
            self.acabar(ahora);
        }
        let (codigo, cola) = (self.codigo.clone(), self.cola.clone());
        let Some(g) = self.acabadas.iter_mut().find(|g| g.destino.is_none()) else {
            return 0;
        };
        if let Some(d) = duracion.filter(|d| *d > 0) {
            g.duracion = Some(d);
        }
        let tope = g.tope(ahora);
        let mut hechas = 0;
        let mut quedan = Vec::new();
        for mut l in std::mem::take(&mut g.cola) {
            l.ms = l.ms.clamp(0, tope);
            l.duracion_ms = l.duracion_ms.min((tope - l.ms).max(0));
            match adjuntar(&destino, &g.titulo, &l) {
                Ok(_) => {
                    let _ = std::fs::remove_file(ficha_de(&l.fichero));
                    hechas += 1;
                }
                // No se tira: sigue en la cola, en su carpeta.
                Err(_) => quedan.push(l),
            }
        }
        g.cola = quedan;
        // **El reloj entra en la nota** (`resto.reloj`): con el, lo que
        // llegue despues por cualquier camino encuentra su minuto por su
        // hora. Y ya no hace falta el apuntado en disco.
        let reloj = RelojDeGrabacion {
            aparato: codigo,
            inicio: g.reloj.desde,
            fin: g.reloj.fin.unwrap_or(ahora),
            pausas: g.reloj.pausas_al_cable(),
            visto: 0,
        };
        if let Some(voz) = &destino.voz {
            let dir = almacen::carpeta(&destino.raiz, &destino.proyecto);
            let puesto = cuaderno::cambiar(&dir, voz, |n| {
                reloj.poner(&mut n.resto);
                true
            });
            if puesto.is_ok_and(|m| m.is_some()) {
                olvidar_reloj(&cola, &g.id);
            }
        }
        g.destino = Some(destino);
        hechas
    }

    /// **Lo que se anuncia al movil**: la de ahora; si no hay, la ultima
    /// acabada (para que sepa que acabo y que aun puede subir lo suyo).
    /// `None` apagada o sin ninguna.
    pub fn anuncio(&self, ahora: i64) -> Option<Anuncio> {
        if !self.encendida {
            return None;
        }
        let (g, activa) = match &self.actual {
            Some(g) => (g, true),
            None => (self.acabadas.first()?, false),
        };
        Some(Anuncio {
            grabacion: g.id.clone(),
            titulo: g.titulo.clone(),
            aparato: self.aparato.clone(),
            activa,
            en_pausa: g.reloj.en_pausa(),
            ms_actual: g.reloj.ms_en(ahora),
            duracion: if activa { 0 } else { g.tope(ahora) },
            desde: g.reloj.desde,
            pausas: g.reloj.pausas_al_cable(),
            codecs: self.codecs.clone(),
            clip_maximo_ms: camara::CLIP_MAXIMO_MS,
            ..Anuncio::nada()
        })
    }

    /// Lo que va adelantado el reloj de `aparato`, medido al avisarle.
    pub fn medir(&mut self, aparato: &str, desfase: i64) {
        self.desfases.insert(aparato.to_string(), desfase);
    }

    fn buscar(&mut self, id: &str) -> Option<&mut Grabacion> {
        self.actual
            .iter_mut()
            .chain(self.acabadas.iter_mut())
            .find(|g| g.id == id)
    }

    /// **Antes del «vale»**: que la grabacion exista aqui y se acepten
    /// fotos.
    pub fn aceptar(&mut self, p: &Suelto) -> Result<(), String> {
        camara::aceptar(p)?;
        if !self.encendida {
            return Err("La cámara del móvil está apagada en el PC".into());
        }
        // Sin grabacion que se conozca, por su hora (se decide al llegar).
        if self.buscar(&p.grabacion).is_none() && p.tomada_en <= 0 {
            return Err("Esa grabación ya no está en el PC".into());
        }
        Ok(())
    }

    /// **Una foto que no sabe de que grabacion es** (o de una que ya no se
    /// recuerda): por su hora, pasada a este reloj con su desfase. En la de
    /// ahora o una acabada si cae dentro; si no, en una guardada de
    /// cualquier chat. `Err` si no cae en ninguna: el movil la deja como una
    /// foto normal.
    fn recibir_por_su_hora(
        &mut self,
        p: &Suelto,
        fichero: &Path,
        de_id: &str,
        de: &str,
        ahora: i64,
    ) -> Result<i64, String> {
        let desfase = if p.reloj != 0 {
            p.reloj
        } else {
            self.desfases.get(de_id).copied().unwrap_or(0)
        };
        let t = p.tomada_en - desfase;
        let en_memoria = self
            .actual
            .iter()
            .chain(self.acabadas.iter())
            .filter_map(|g| {
                let r = self.reloj_de(g, ahora);
                let ms = por_su_hora::lugar(t, &r, g.duracion, por_su_hora::MARGEN_MS)?;
                Some((g.id.clone(), r.inicio, ms))
            })
            .max_by_key(|(_, inicio, _)| *inicio);
        if let Some((id, _, ms)) = en_memoria {
            let p = Suelto {
                grabacion: id,
                ms: Some(ms),
                ..p.clone()
            };
            return self.recibir(&p, fichero, de_id, de, ahora);
        }
        let (raiz, hora) = self.almacen.clone().ok_or(NINGUNA)?;
        let (proyecto, g, ms) =
            por_su_hora::buscar_por_hora(&raiz, t, &self.codigo).ok_or(NINGUNA)?;
        let clip = camara::es_clip(p);
        let tope = g.reloj.duracion(ahora).max(g.duracion_ms);
        let l = Llegada {
            ms,
            duracion_ms: if clip {
                camara::duracion_del_clip(p, ms, tope)
            } else {
                0
            },
            clip,
            fichero: fichero.to_path_buf(),
            nombre: nombre_de(p, clip, 1),
            de: de.to_string(),
            tomada_en: p.tomada_en,
            clave: p.clave.clone(),
        };
        let destino = Destino {
            raiz,
            proyecto,
            voz: Some(g.voz.clone()),
            carpeta: g.carpeta.clone(),
            aparato: self.codigo.clone(),
            hora,
        };
        adjuntar(&destino, &g.titulo, &l).map_err(|e| format!("No se pudo guardar: {e}"))?;
        self.nuevas.push(Recibida {
            ms,
            clip,
            de: de.to_string(),
        });
        Ok(ms)
    }

    /// **Una foto o un clip llego** a `fichero` (temporal) de `de`
    /// (`de_id`, para su desfase). Entra ya si la clase esta guardada; si
    /// no, espera en la cola. Devuelve su minuto.
    pub fn recibir(
        &mut self,
        p: &Suelto,
        fichero: &Path,
        de_id: &str,
        de: &str,
        ahora: i64,
    ) -> Result<i64, String> {
        self.aceptar(p)?;
        if self.buscar(&p.grabacion).is_none() {
            return self.recibir_por_su_hora(p, fichero, de_id, de, ahora);
        }
        let medido = self.desfases.get(de_id).copied();
        let cola = self.cola.clone();
        let g = self
            .buscar(&p.grabacion)
            .ok_or_else(|| "Esa grabación ya no está en el PC".to_string())?;
        let tope = g.tope(ahora);
        let ms = camara::ms_de_la_foto(&g.reloj, ahora, Some(tope), p, medido);
        let clip = camara::es_clip(p);
        let duracion_ms = if clip {
            camara::duracion_del_clip(p, ms, tope)
        } else {
            0
        };
        let nombre = nombre_de(p, clip, g.recibidas + 1);
        let mut l = Llegada {
            ms,
            duracion_ms,
            clip,
            fichero: fichero.to_path_buf(),
            nombre,
            de: de.to_string(),
            tomada_en: p.tomada_en,
            clave: p.clave.clone(),
        };
        match g.destino.clone() {
            Some(d) => {
                adjuntar(&d, &g.titulo, &l).map_err(|e| format!("No se pudo guardar: {e}"))?;
            }
            None => {
                let dir = carpeta_de_la_cola(&cola, &g.id);
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let sitio = dir.join(format!("{}-{}", g.cola.len(), l.nombre));
                mover(fichero, &sitio).map_err(|e| format!("No se pudo guardar: {e}"))?;
                l.fichero = sitio;
                // La ficha, en el acto: un cierre de golpe antes de guardar
                // la clase no se lleva su minuto.
                // Sin ficha (el disco), la foto sigue en la cola de memoria:
                // solo un corte antes de guardar la dejaria sin su minuto.
                let _ = apuntar(&l);
                g.cola.push(l);
            }
        }
        g.recibidas += 1;
        self.nuevas.push(Recibida {
            ms,
            clip,
            de: de.to_string(),
        });
        Ok(ms)
    }

    /// Lo llegado desde la ultima vez que se pregunto.
    pub fn tomar_nuevas(&mut self) -> Vec<Recibida> {
        std::mem::take(&mut self.nuevas)
    }
}

/// Lo que se contesta a una foto cuya hora no cae en ninguna grabacion.
const NINGUNA: &str = "La foto no cae en ninguna grabación de este equipo";

fn fichero_del_reloj(cola: &Path, id: &str) -> PathBuf {
    cola.join("relojes")
        .join(format!("{}.json", almacen::nombre_seguro(id)))
}

/// **Los relojes apuntados de grabaciones que no llegaron a guardarse** (el
/// programa se cayo grabando): el id y su ventana, hasta lo ultimo visto.
/// Quien recupera la grabacion pone el suyo en la nota (`resto.reloj`) y
/// borra el fichero ([`olvidar_reloj`]).
pub fn relojes_sueltos(cola: &Path) -> Vec<(String, RelojDeGrabacion)> {
    let Ok(d) = std::fs::read_dir(cola.join("relojes")) else {
        return Vec::new();
    };
    d.filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                return None;
            }
            let r: RelojDeGrabacion = serde_json::from_slice(&std::fs::read(&p).ok()?).ok()?;
            let id = p.file_stem()?.to_string_lossy().to_string();
            Some((id, r))
        })
        .collect()
}

/// Borra el reloj apuntado de la grabacion `id` (ya esta en su nota).
pub fn olvidar_reloj(cola: &Path, id: &str) {
    let _ = std::fs::remove_file(fichero_del_reloj(cola, id));
}

/// La ficha de una llegada en cola: `<fichero>.json`.
fn ficha_de(fichero: &Path) -> PathBuf {
    let mut s = fichero.as_os_str().to_os_string();
    s.push(".json");
    PathBuf::from(s)
}

fn apuntar(l: &Llegada) -> std::io::Result<()> {
    let json = serde_json::to_vec(l).map_err(std::io::Error::other)?;
    crate::grabacion_a_medias::escribir_entero(&ficha_de(&l.fichero), &json)
}

/// La carpeta de la cola de la grabacion `grabacion`, dentro de `cola`.
pub fn carpeta_de_la_cola(cola: &Path, grabacion: &str) -> PathBuf {
    cola.join(almacen::nombre_seguro(grabacion))
}

/// **Lo que espera en la cola de `grabacion`**, leido del disco (sus
/// fichas), en el orden de la grabacion. Lo que no tiene ficha o ya no tiene
/// fichero no sale.
pub fn cola_en_disco(cola: &Path, grabacion: &str) -> Vec<Llegada> {
    let Ok(entradas) = std::fs::read_dir(carpeta_de_la_cola(cola, grabacion)) else {
        return Vec::new();
    };
    let mut v: Vec<Llegada> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| serde_json::from_slice::<Llegada>(&std::fs::read(&p).ok()?).ok())
        .filter(|l| l.fichero.is_file())
        .collect();
    v.sort_by_key(|l| (l.ms, l.tomada_en));
    v
}

/// **Mete en la clase recuperada** `destino` lo que llego del movil a la
/// grabacion `grabacion` antes del corte, en su minuto (dentro de los
/// `duracion_ms` de la nota). Lo que no entra se queda en la cola. Devuelve
/// cuantas entraron.
pub fn recuperar_cola(
    destino: &Destino,
    cola: &Path,
    grabacion: &str,
    titulo: &str,
    duracion_ms: i64,
) -> usize {
    let tope = duracion_ms.max(0);
    let mut hechas = 0;
    for mut l in cola_en_disco(cola, grabacion) {
        let ficha = ficha_de(&l.fichero);
        l.ms = l.ms.clamp(0, tope);
        l.duracion_ms = l.duracion_ms.min((tope - l.ms).max(0));
        if adjuntar(destino, titulo, &l).is_ok() {
            let _ = std::fs::remove_file(ficha);
            hechas += 1;
        }
    }
    let _ = std::fs::remove_dir(carpeta_de_la_cola(cola, grabacion));
    hechas
}

/// Tira lo que espera en la cola de `grabacion` (se descarto).
pub fn tirar_cola_en_disco(cola: &Path, grabacion: &str) {
    let _ = std::fs::remove_dir_all(carpeta_de_la_cola(cola, grabacion));
}

fn tirar_cola(g: &Grabacion) {
    for l in &g.cola {
        let _ = std::fs::remove_file(&l.fichero);
        let _ = std::fs::remove_file(ficha_de(&l.fichero));
    }
    if let Some(dir) = g.cola.first().and_then(|l| l.fichero.parent()) {
        let _ = std::fs::remove_dir(dir);
    }
}

fn mover(desde: &Path, hasta: &Path) -> std::io::Result<()> {
    if std::fs::rename(desde, hasta).is_err() {
        std::fs::copy(desde, hasta)?;
        let _ = std::fs::remove_file(desde);
    }
    Ok(())
}

/// El nombre con que entra: el suyo si lo trae con una extension que cuadra;
/// si no, `movil-N.jpg` / `.mp4`.
fn nombre_de(p: &Suelto, clip: bool, n: usize) -> String {
    let suyo = Path::new(&p.nombre)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let cuadra = pixpin_sincro::al_lienzo::mime_de(&suyo) == p.mime;
    if cuadra && !suyo.trim().is_empty() {
        return suyo;
    }
    let ext = match p.mime.as_str() {
        "image/png" => "png",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        _ if clip => "mp4",
        _ => "jpg",
    };
    format!("movil-{n}.{ext}")
}

/// El minuto como en el pie de las capturas: `12:34` o `1:02:03`.
pub fn minuto(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// **Pone una foto o un clip del movil en la clase guardada** `destino`:
/// el mensaje (con el fichero movido dentro del proyecto, tal cual llego),
/// su enlace a la nota, su sitio en la lista de la nota y en la carpeta.
/// Devuelve el id del mensaje.
pub fn adjuntar(destino: &Destino, titulo: &str, l: &Llegada) -> std::io::Result<String> {
    let raiz = &destino.raiz;
    let proyecto = &destino.proyecto;
    let carpeta = almacen::carpeta(raiz, proyecto);
    let leido = match cuaderno::Cuaderno::leer_de(&carpeta) {
        Ok(c) => Some(c),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let numero = leido
        .as_ref()
        .map_or(1, |c| c.siguiente_numero(Some(proyecto)));
    // **La misma foto otra vez** (el movil la reintento, o ya llego por la
    // sincronizacion): no se copia de nuevo.
    if let Some(c) = &leido
        && let Some(id) = ya_llego(c, destino, l)?
    {
        let _ = std::fs::remove_file(&l.fichero);
        return Ok(id);
    }
    let (ruta, bytes) = almacen::mover_adjunto(raiz, proyecto, &l.nombre, &l.fichero)?;
    let cuando = (destino.hora)();
    let mut m = cuaderno::Mensaje::adjunto(
        cuaderno::clase_de_nombre(&l.nombre),
        &l.nombre,
        &ruta,
        bytes,
        &cuaderno::Sello {
            cuando,
            numero,
            aparato: destino.aparato.clone(),
            proyecto: proyecto.clone(),
        },
    );
    // Como lo de la camara del equipo: «⏱» una foto, «▶» un clip.
    let rotulo = format!("📱 {} {}", if l.clip { "▶" } else { "⏱" }, minuto(l.ms));
    m.texto = if titulo.trim().is_empty() {
        rotulo.clone()
    } else {
        format!("{rotulo} · {}", titulo.trim())
    };
    if let Some(voz) = &destino.voz {
        let enlace = if l.clip {
            presencial::enlace_del_clip(voz, l.ms, l.duracion_ms)
        } else {
            serde_json::json!({ "de": voz, "ms": l.ms })
        };
        m.resto.insert("enGrabacion".into(), enlace);
    }
    presencial::marcar_del_movil(&mut m.resto);
    if !l.de.is_empty() {
        m.resto
            .insert("deAparato".into(), serde_json::json!(l.de.clone()));
    }
    if l.tomada_en > 0 {
        m.resto
            .insert("tomadaEn".into(), serde_json::json!(l.tomada_en));
    }
    if !l.clave.trim().is_empty() {
        m.resto
            .insert(por_su_hora::FOTO_UID.into(), serde_json::json!(l.clave));
    }
    cuaderno::anadir(&carpeta, &m)?;
    let id = m.id.clone();
    if let Some(voz) = &destino.voz {
        cuaderno::cambiar(&carpeta, voz, |n| {
            if l.clip {
                // La misma lista que los clips de la camara del equipo.
                let mut clips = presencial::clips_de(&n.resto);
                if !clips.iter().any(|c| c.id == id) {
                    clips.push(presencial::Clip {
                        ms: l.ms,
                        duracion_ms: l.duracion_ms,
                        id: id.clone(),
                    });
                }
                presencial::poner_clips(&mut n.resto, &clips);
            } else {
                clase::poner_captura(&mut n.resto, l.ms, &id);
            }
            true
        })?;
    }
    if let Some(c) = &destino.carpeta {
        carpeta_de_paginas::cambiar_en_disco(&carpeta, c, |c| {
            c.anadir_en_su_minuto(&id, &rotulo, l.ms)
        })?;
    }
    let mut indice = almacen::Indice::leer(raiz);
    if let Some(f) = indice.proyectos.iter_mut().find(|f| &f.id == proyecto) {
        f.tocado = f.tocado.max(cuando);
        indice.guardar(raiz)?;
    }
    Ok(id)
}

/// **Si la foto `l` ya esta en el chat**: copiada antes por la camara (su
/// `fotoUid` es la `clave`) o llegada por la sincronizacion (su uid es la
/// `clave`; entonces se pone esa en la grabacion, sin copiarla). El id del
/// mensaje que la tiene.
fn ya_llego(
    c: &cuaderno::Cuaderno,
    destino: &Destino,
    l: &Llegada,
) -> std::io::Result<Option<String>> {
    let clave = l.clave.trim();
    if clave.is_empty() {
        return Ok(None);
    }
    if let Some(m) = c
        .mensajes
        .iter()
        .find(|m| m.resto.get(por_su_hora::FOTO_UID).and_then(|v| v.as_str()) == Some(clave))
    {
        return Ok(Some(m.id.clone()));
    }
    let Some(m) = c
        .mensajes
        .iter()
        .find(|m| !l.clip && m.uid.as_deref() == Some(clave))
    else {
        return Ok(None);
    };
    if let Some(voz) = &destino.voz {
        let f = por_su_hora::FotoConHora {
            id: m.id.clone(),
            uid: m.codigo_unico(),
            aparato: m.aparato.clone().unwrap_or_default(),
            tomada_en: l.tomada_en,
            para: None,
        };
        let g = por_su_hora::Grabacion {
            voz: voz.clone(),
            carpeta: destino.carpeta.clone(),
            ..Default::default()
        };
        let dir = almacen::carpeta(&destino.raiz, &destino.proyecto);
        por_su_hora::colocar(&dir, &g, &f, l.ms, false, true)?;
    }
    Ok(Some(m.id.clone()))
}

// ------------------------------------------------------------ al cable

/// **Lo que atiende la camara** en una conexion con `de` (lo da quien
/// recibe los sueltos, ver `pixpin_sincro::al_lienzo::Recibe::camara`).
pub struct Recepcion {
    pub camaras: Arc<Mutex<Camaras>>,
    pub de_id: String,
    pub de_nombre: String,
    pub ahora: fn() -> i64,
}

impl Recepcion {
    fn con<T>(&self, f: impl FnOnce(&mut Camaras) -> T) -> T {
        let mut c = self.camaras.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut c)
    }
}

impl camara::Camara for Recepcion {
    fn anuncio(&mut self) -> Option<Anuncio> {
        let ahora = (self.ahora)();
        self.con(|c| c.anuncio(ahora))
    }

    fn aceptar_foto(&mut self, p: &Suelto) -> Result<(), String> {
        self.con(|c| c.aceptar(p))
    }

    fn guardar_foto(&mut self, p: &Suelto, fichero: &Path) -> Result<i64, String> {
        let ahora = (self.ahora)();
        let (id, nombre) = (self.de_id.clone(), self.de_nombre.clone());
        self.con(|c| c.recibir(p, fichero, &id, &nombre, ahora))
    }

    fn anuncio_ajeno(&mut self, de: &pixpin_sincro::mensajes::Aparato, a: &Anuncio, desfase: i64) {
        let ahora = (self.ahora)();
        self.con(|c| c.apuntar_ajena(&de.id, &de.nombre, a, desfase, ahora));
    }
}

/// Un `Recibe` que solo atiende la camara: lo demas («suelto» al chat
/// abierto) lo niega. Para quien no tiene otra cosa que hacer con los
/// sueltos (las pruebas, una herramienta).
impl pixpin_sincro::al_lienzo::Recibe for Recepcion {
    fn aceptar(&mut self, _: &Suelto) -> Result<(), String> {
        Err(format!(
            "No sé qué es «{}»",
            pixpin_sincro::al_lienzo::SUELTO
        ))
    }

    fn guardar(
        &mut self,
        _: &Suelto,
        _: &Path,
    ) -> Result<pixpin_sincro::al_lienzo::Llegada, String> {
        Err(format!(
            "No sé qué es «{}»",
            pixpin_sincro::al_lienzo::SUELTO
        ))
    }

    fn camara(&mut self) -> Option<&mut dyn camara::Camara> {
        Some(self)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const S: i64 = 1000;
    const PARED: i64 = 1_761_550_000_000;

    fn foto(grabacion: &str, ms: Option<i64>) -> Suelto {
        Suelto {
            nombre: "IMG_1.jpg".into(),
            mime: "image/jpeg".into(),
            bytes: 4,
            destino: camara::DESTINO.into(),
            grabacion: grabacion.into(),
            ms,
            ..Default::default()
        }
    }

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-camara-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join(nombre);
        std::fs::write(&f, [0xFF, 0xD8, 0xFF, 0xE1]).unwrap();
        f
    }

    #[test]
    fn el_anuncio_dice_en_que_minuto_va_y_si_esta_en_pausa() {
        let dir = std::env::temp_dir().join(format!("pixpin-camara-a-{}", std::process::id()));
        let mut c = Camaras::nuevas(dir, "Portatil");
        assert_eq!(c.anuncio(PARED), None);
        c.empezar("g1", "Clase 27-10", PARED, 0);
        assert!(!c.seguir_a(PARED + 10 * S, 10 * S, false));
        assert!(c.seguir_a(PARED + 12 * S, 12 * S, true), "cambio la pausa");
        let a = c.anuncio(PARED + 20 * S).unwrap();
        assert!(a.grabando() && a.en_pausa);
        assert_eq!(a.ms_actual, 12 * S);
        assert_eq!(a.aparato, "Portatil");
        assert_eq!(a.codec_del_clip(), Some(camara::H264));
        c.acabar(PARED + 30 * S);
        let a = c.anuncio(PARED + 40 * S).unwrap();
        assert!(!a.activa, "acabo, pero se dice cual era");
        assert_eq!((a.grabacion.as_str(), a.duracion), ("g1", 12 * S));
        // Apagada, nada.
        c.encendida = false;
        assert_eq!(c.anuncio(PARED + 40 * S), None);
    }

    #[test]
    fn caso_negativo_lo_de_otra_grabacion_o_apagada_no_entra() {
        let dir = std::env::temp_dir().join(format!("pixpin-camara-b-{}", std::process::id()));
        let mut c = Camaras::nuevas(dir, "Portatil");
        c.empezar("g1", "", PARED, 0);
        assert!(c.aceptar(&foto("g1", Some(1))).is_ok());
        assert!(c.aceptar(&foto("otra", Some(1))).is_err());
        let f = temporal("x.jpg");
        assert!(
            c.recibir(&foto("otra", None), &f, "tel", "Tel", PARED)
                .is_err()
        );
        c.encendida = false;
        assert!(c.aceptar(&foto("g1", Some(1))).is_err());
        assert!(c.tomar_nuevas().is_empty());
        // Tirada, lo que esperaba se va.
        c.encendida = true;
        let ms = c.recibir(&foto("g1", Some(500)), &f, "tel", "Tel", PARED + S);
        assert_eq!(ms, Ok(500));
        assert_eq!(c.recibidas(), 1);
        c.tirar();
        assert_eq!(c.en_curso(), None);
        assert!(c.aceptar(&foto("g1", Some(1))).is_err());
    }

    #[test]
    fn la_cola_sobrevive_a_un_cierre_de_golpe_y_se_recupera_en_su_minuto() {
        let base = std::env::temp_dir().join(format!("pixpin-camara-c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let cola = base.join("cola");
        let mut c = Camaras::nuevas(cola.clone(), "Portatil");
        c.empezar("g-7", "Clase 28-10", PARED, 0);
        let f = temporal("cola-1.jpg");
        assert_eq!(
            c.recibir(&foto("g-7", Some(90 * S)), &f, "tel", "Tel", PARED + 95 * S),
            Ok(90 * S)
        );
        let f = temporal("cola-2.jpg");
        assert_eq!(
            c.recibir(&foto("g-7", Some(30 * S)), &f, "tel", "Tel", PARED + 96 * S),
            Ok(30 * S)
        );
        // **El corte**: la memoria se va sin `acabar` ni `guardada`.
        drop(c);

        let en_disco = cola_en_disco(&cola, "g-7");
        assert_eq!(
            en_disco.iter().map(|l| l.ms).collect::<Vec<_>>(),
            vec![30 * S, 90 * S]
        );
        assert!(en_disco.iter().all(|l| l.fichero.is_file()));
        // Caso negativo: la cola de otra grabacion esta vacia.
        assert!(cola_en_disco(&cola, "otra").is_empty());

        // Se recupera en una nota que duro un minuto: la de 1:30 cae al
        // final, no fuera.
        let raiz = base.join("raiz");
        let destino = Destino {
            raiz: raiz.clone(),
            proyecto: "p1".into(),
            voz: None,
            carpeta: None,
            aparato: "K7Q2".into(),
            hora: || PARED,
        };
        assert_eq!(recuperar_cola(&destino, &cola, "g-7", "Clase", 60 * S), 2);
        assert!(cola_en_disco(&cola, "g-7").is_empty());
        let mensajes = cuaderno::Cuaderno::leer_de(&almacen::carpeta(&raiz, "p1"))
            .unwrap()
            .mensajes;
        assert_eq!(mensajes.len(), 2);
        assert!(mensajes.iter().all(es_del_movil));
        assert!(mensajes.iter().any(|m| m.texto.contains("1:00")));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn el_nombre_es_el_suyo_si_cuadra_y_si_no_uno_de_serie() {
        let mut p = foto("g", None);
        assert_eq!(nombre_de(&p, false, 3), "IMG_1.jpg");
        p.nombre = "../../malo.exe".into();
        assert_eq!(nombre_de(&p, false, 3), "movil-3.jpg");
        p.nombre = "clip.mp4".into();
        p.mime = "video/mp4".into();
        assert_eq!(nombre_de(&p, true, 4), "clip.mp4");
        assert_eq!(minuto(754_000), "12:34");
        assert_eq!(minuto(3_723_000), "1:02:03");
    }
}
