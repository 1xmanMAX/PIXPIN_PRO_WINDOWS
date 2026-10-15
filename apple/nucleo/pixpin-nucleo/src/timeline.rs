//! **El timeline**: los dias con algo y sus momentos, por paginas.
//!
//! No se inventa nada: es `pixpin_timeline` tal cual (el de Windows). Los
//! momentos viven en `<datos>/timeline/momentos.jsonl` y sus fotos y audios
//! en `<datos>/timeline/medios/` (`pixpin_timeline::almacen::Almacen`); los
//! dias se cuentan con `pixpin_timeline::dias` en el huso de quien mira (el
//! `desfase_ms` que pasa Swift: `TimeZone.current.secondsFromGMT() * 1000`).
//!
//! # Rendimiento
//!
//! Un ano de notas son pocos cientos de KB, pero no se relee en cada pagina:
//! los momentos leidos se guardan en una cache que vale mientras el fichero
//! tenga la misma fecha y el mismo tamano (`Almacen::huella`), y se suelta
//! con [`Nucleo::vaciar_caches`]. Swift pide primero los dias (una lista
//! corta) y despues los momentos de un dia, de pagina en pagina.

use std::sync::Arc;
use std::time::SystemTime;

use pixpin_timeline::Momento;
use pixpin_timeline::almacen::Almacen;
use pixpin_timeline::dias::{self, Dia};

use crate::{ErrorNucleo, Nucleo};

/// Lo mas que se devuelve en una pagina del timeline.
pub const TOPE_DE_PAGINA_TIMELINE: u32 = 500;

/// Un dia con momentos.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DiaTimeline {
    pub anio: i32,
    pub mes: u8,
    pub dia: u8,
    /// `AAAA-MM-DD`.
    pub iso: String,
    /// Cuantos momentos tiene.
    pub cuantos: u32,
}

/// Una pagina de dias, del mas nuevo al mas viejo.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaDias {
    pub dias: Vec<DiaTimeline>,
    pub desde: u64,
    pub total: u64,
}

/// Un momento, listo para pintar su tarjeta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MomentoTimeline {
    pub id: String,
    /// Milisegundos desde 1970 (UTC).
    pub cuando: i64,
    /// `HH:MM` en el huso de quien mira.
    pub hora: String,
    pub titulo: String,
    pub descripcion: String,
    /// Las fotos, con su ruta absoluta, en orden.
    pub fotos: Vec<String>,
    /// El audio de lo dicho, con su ruta absoluta.
    pub audio: Option<String>,
    pub duracion_ms: i64,
    /// Mandado a «Momentos».
    pub en_momentos: bool,
    /// La leccion que se hizo de este momento, si se hizo.
    pub leccion: Option<String>,
}

/// Una pagina de momentos de un dia, del mas viejo al mas nuevo (como se
/// leen en la linea de tiempo).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PaginaMomentos {
    pub momentos: Vec<MomentoTimeline>,
    pub desde: u64,
    pub total: u64,
}

/// La huella del fichero de los momentos (`Almacen::huella`).
type Huella = Option<(SystemTime, u64)>;

/// Los momentos de la ultima lectura y la huella del fichero de entonces.
#[derive(Debug, Default)]
pub struct CacheTimeline {
    leidos: Option<(Huella, Arc<Vec<Momento>>)>,
}

impl CacheTimeline {
    pub(crate) fn vaciar(&mut self) {
        self.leidos = None;
    }
}

fn a_ficha(a: &Almacen, m: &Momento, desfase_ms: i64) -> MomentoTimeline {
    MomentoTimeline {
        id: m.id.clone(),
        cuando: m.cuando,
        hora: dias::hora(m.cuando, desfase_ms),
        titulo: m.titulo.clone(),
        descripcion: m.descripcion.clone(),
        fotos: m
            .fotos
            .iter()
            .filter(|f| nombre_de_medio_valido(f))
            .map(|f| a.ruta(f).display().to_string())
            .collect(),
        audio: m
            .audio
            .as_ref()
            .filter(|f| nombre_de_medio_valido(f))
            .map(|f| a.ruta(f).display().to_string()),
        duracion_ms: m.duracion_ms,
        en_momentos: m.en_momentos,
        leccion: m.leccion.clone(),
    }
}

/// Un medio es un nombre suelto dentro de `medios/`: nada de rutas (el
/// fichero puede venir de otro aparato).
fn nombre_de_medio_valido(n: &str) -> bool {
    !n.is_empty() && n != "." && n != ".." && !n.contains(['/', '\\', '\0'])
}

impl Nucleo {
    /// Todos los momentos, de la cache si el fichero no cambio.
    pub(crate) fn momentos(&self) -> Arc<Vec<Momento>> {
        let a = Almacen::en(self.ruta());
        let huella = a.huella();
        let mut cache = self.timeline_tomada();
        if let Some((h, v)) = &cache.leidos
            && huella.is_some()
            && *h == huella
        {
            return Arc::clone(v);
        }
        let v = Arc::new(a.leer());
        cache.leidos = Some((huella, Arc::clone(&v)));
        v
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Los dias con momentos**, del mas nuevo al mas viejo, por paginas.
    /// Con `solo_en_momentos`, solo cuentan los mandados a «Momentos» (la
    /// pestana del archivo); sin el, todos (la de «Hoy» y «Estado»).
    pub fn timeline_dias(
        &self,
        desfase_ms: i64,
        solo_en_momentos: bool,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaDias, ErrorNucleo> {
        let todos = self.momentos();
        let elegidos: Vec<Momento>;
        let lista: &[Momento] = if solo_en_momentos {
            elegidos = todos.iter().filter(|m| m.en_momentos).cloned().collect();
            &elegidos
        } else {
            &todos
        };
        let con = dias::dias_con(lista, desfase_ms);
        let total = con.len();
        let desde = (desde.min(total as u64)) as usize;
        let cuantos = cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize;
        let dias = con
            .iter()
            .rev()
            .skip(desde)
            .take(cuantos)
            .map(|(d, n)| DiaTimeline {
                anio: d.anio,
                mes: d.mes,
                dia: d.dia,
                iso: d.iso(),
                cuantos: *n as u32,
            })
            .collect();
        Ok(PaginaDias {
            dias,
            desde: desde as u64,
            total: total as u64,
        })
    }

    /// **Los momentos de un dia**, del mas viejo al mas nuevo, por paginas.
    /// Un dia que no existe (30 de febrero) es un error de argumento.
    #[allow(clippy::too_many_arguments)]
    pub fn timeline_momentos(
        &self,
        anio: i32,
        mes: u8,
        dia: u8,
        desfase_ms: i64,
        solo_en_momentos: bool,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMomentos, ErrorNucleo> {
        let d = Dia { anio, mes, dia };
        if !(1..=12).contains(&mes) || dia == 0 || Dia::de_numero(d.numero()) != d {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("dia que no existe: {anio}-{mes}-{dia}"),
            });
        }
        let todos = self.momentos();
        let del_dia: Vec<&Momento> = dias::del_dia(&todos, d, desfase_ms)
            .into_iter()
            .filter(|m| !solo_en_momentos || m.en_momentos)
            .collect();
        Ok(pagina(self, &del_dia, desfase_ms, desde, cuantos))
    }

    /// **Hoy**: lo de las ultimas 24 horas, del mas viejo al mas nuevo (lo
    /// nuevo abajo, como en un chat), por paginas.
    pub fn timeline_ultimas_24h(
        &self,
        desfase_ms: i64,
        desde: u64,
        cuantos: u32,
    ) -> Result<PaginaMomentos, ErrorNucleo> {
        let todos = self.momentos();
        let v = dias::ultimas_24h(&todos, crate::ahora_ms());
        Ok(pagina(self, &v, desfase_ms, desde, cuantos))
    }

    /// Busca en el titulo y la descripcion de TODOS los momentos, sin
    /// mayusculas ni tildes y con cada palabra en cualquier orden
    /// (`pixpin_timeline::buscar`), del mas nuevo al mas viejo.
    pub fn timeline_buscar(
        &self,
        consulta: String,
        desfase_ms: i64,
        cuantos: u32,
    ) -> Result<Vec<MomentoTimeline>, ErrorNucleo> {
        let todos = self.momentos();
        let a = Almacen::en(self.ruta());
        Ok(pixpin_timeline::buscar::buscar(&todos, &consulta)
            .into_iter()
            .take(cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize)
            .map(|m| a_ficha(&a, m, desfase_ms))
            .collect())
    }
}

fn pagina(n: &Nucleo, v: &[&Momento], desfase_ms: i64, desde: u64, cuantos: u32) -> PaginaMomentos {
    let a = Almacen::en(n.ruta());
    let total = v.len();
    let desde = (desde.min(total as u64)) as usize;
    let hasta = (desde + cuantos.min(TOPE_DE_PAGINA_TIMELINE) as usize).min(total);
    PaginaMomentos {
        momentos: v[desde..hasta]
            .iter()
            .map(|m| a_ficha(&a, m, desfase_ms))
            .collect(),
        desde: desde as u64,
        total: total as u64,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_medio_con_rutas_no_vale() {
        assert!(nombre_de_medio_valido("tl-1-0-1.png"));
        assert!(!nombre_de_medio_valido("../indice.json"));
        assert!(!nombre_de_medio_valido("a/b.png"));
        assert!(!nombre_de_medio_valido(""));
    }
}
