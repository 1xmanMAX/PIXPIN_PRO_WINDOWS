//! El almacen: la verdad de todo lo pineado (D21, D25, D27).
//!
//! Ficheros reales navegables con el Explorador mas un indice JSON que es
//! lo UNICO que se reescribe — siempre a temporal + rename, la disciplina
//! que el ExcalidrawStore del Android aprendio a golpes. Los objetos se
//! crean con contador y no se tocan jamas; solo "Eliminar del almacen"
//! (S2-B) los borrara.
//!
//! Este modulo es std::fs puro: se prueba entero con directorios
//! temporales, sin Windows.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ErrorAlmacen {
    #[error("no se pudo acceder a {1}: {0}")]
    Io(#[source] std::io::Error, PathBuf),
    #[error("el indice del almacen tiene un error: {0}")]
    Indice(#[from] serde_json::Error),
    #[error("no existe ninguna entrada con id {0}")]
    NoExiste(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinGuardado {
    pub x: i32,
    pub y: i32,
    pub ancho: u32,
    pub alto: u32,
    /// DPI del monitor donde vivia, para restaurar con sentido (spec 5.2).
    pub escala_por_cien: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoEntrada {
    Imagen,
    // Nota y Archivo llegan en S2-B; el serde tolerante ya los aguantara.
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entrada {
    pub id: u64,
    pub tipo: TipoEntrada,
    /// ISO-8601 UTC, texto: el indice se lee con un editor.
    pub creado: String,
    pub origen: String,
    /// Ruta relativa a `almacen/`.
    pub objeto: String,
    pub grupo: Option<u32>,
    pub pin: Option<PinGuardado>,
}

/// El fichero indice.json entero. `#[serde(default)]` en todo: un indice de
/// una version futura abre igual (misma regla que los ajustes).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Indice {
    #[serde(default = "version_uno")]
    version: u32,
    #[serde(default)]
    siguiente_id: u64,
    #[serde(default)]
    entradas: Vec<Entrada>,
}

fn version_uno() -> u32 {
    1
}

pub struct Almacen {
    dir: PathBuf,
    indice: Indice,
}

impl Almacen {
    pub fn abrir(raiz: &Path) -> Result<Almacen, ErrorAlmacen> {
        let dir = raiz.join("almacen");
        fs::create_dir_all(dir.join("objetos")).map_err(|e| ErrorAlmacen::Io(e, dir.clone()))?;
        let ruta = dir.join("indice.json");
        let indice = match fs::read_to_string(&ruta) {
            Ok(texto) => serde_json::from_str(&texto)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Indice {
                version: 1,
                siguiente_id: 1,
                entradas: Vec::new(),
            },
            Err(e) => return Err(ErrorAlmacen::Io(e, ruta)),
        };
        Ok(Almacen { dir, indice })
    }

    pub fn entradas(&self) -> &[Entrada] {
        &self.indice.entradas
    }

    pub fn ruta_objeto(&self, e: &Entrada) -> PathBuf {
        self.dir.join(&e.objeto)
    }

    pub fn guardar_imagen(
        &mut self,
        png: &[u8],
        origen: &str,
        pin: Option<PinGuardado>,
    ) -> Result<u64, ErrorAlmacen> {
        let id = self.indice.siguiente_id.max(1);
        self.indice.siguiente_id = id + 1;

        let (anio, mes) = anio_mes_utc();
        let relativa = format!("objetos/{anio:04}/{mes:02}/{id:06}.png");
        let ruta = self.dir.join(&relativa);
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre).map_err(|e| ErrorAlmacen::Io(e, padre.to_path_buf()))?;
        }
        // El objeto se escribe UNA vez y no se toca mas.
        fs::write(&ruta, png).map_err(|e| ErrorAlmacen::Io(e, ruta.clone()))?;

        self.indice.entradas.push(Entrada {
            id,
            tipo: TipoEntrada::Imagen,
            creado: ahora_iso(),
            origen: origen.to_string(),
            objeto: relativa,
            grupo: None,
            pin,
        });
        self.persistir()?;
        Ok(id)
    }

    pub fn actualizar_pin(
        &mut self,
        id: u64,
        pin: Option<PinGuardado>,
    ) -> Result<(), ErrorAlmacen> {
        let entrada = self
            .indice
            .entradas
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or(ErrorAlmacen::NoExiste(id))?;
        entrada.pin = pin;
        self.persistir()
    }

    /// Temporal + rename: un proceso que muera a mitad deja el indice
    /// anterior intacto, nunca uno a medias que no abre.
    fn persistir(&self) -> Result<(), ErrorAlmacen> {
        let definitivo = self.dir.join("indice.json");
        let temporal = self.dir.join("indice.json.tmp");
        let texto = serde_json::to_string_pretty(&self.indice)?;
        fs::write(&temporal, texto).map_err(|e| ErrorAlmacen::Io(e, temporal.clone()))?;
        fs::rename(&temporal, &definitivo).map_err(|e| ErrorAlmacen::Io(e, definitivo))?;
        Ok(())
    }
}

/// (año, mes) actuales en UTC, sin dependencias: dias desde epoch con el
/// algoritmo civil de Howard Hinnant, suficiente y determinista.
fn anio_mes_utc() -> (i64, u32) {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let (anio, mes, _dia) = civil_desde_dias(dias);
    (anio, mes)
}

fn ahora_iso() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);
    let (a, m, d) = civil_desde_dias(dias);
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
}

/// Conversion dias-desde-epoch -> fecha civil (Hinnant, dominio ±millones
/// de años; aqui solo se usa con fechas reales).
fn civil_desde_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::fs;

    fn raiz(etiqueta: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("pixpin-almacen-{etiqueta}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Un PNG minimo valido no hace falta: el almacen guarda BYTES y no
    /// valida el formato (eso es del codec). Cuatro bytes bastan.
    const BYTES: &[u8] = &[0x89, b'P', b'N', b'G'];

    fn pin() -> PinGuardado {
        PinGuardado {
            x: 100,
            y: 200,
            ancho: 300,
            alto: 150,
            escala_por_cien: 150,
        }
    }

    #[test]
    fn guardar_crea_objeto_e_indice_y_sobrevive_a_reabrir() {
        let r = raiz("basico");
        let id = {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap()
        };
        // Reabrir desde disco: nada vive solo en memoria (D25).
        let a = Almacen::abrir(&r).unwrap();
        let e = a
            .entradas()
            .iter()
            .find(|e| e.id == id)
            .expect("la entrada persiste");
        assert_eq!(e.tipo, TipoEntrada::Imagen);
        assert_eq!(e.origen, "recorte");
        assert_eq!(e.pin, Some(pin()));
        // El objeto es un fichero real navegable (D27).
        assert_eq!(fs::read(a.ruta_objeto(e)).unwrap(), BYTES);
    }

    #[test]
    fn cerrar_un_pin_no_borra_nada() {
        // D21 como test: actualizar a None conserva entrada y objeto.
        let r = raiz("cerrar");
        let mut a = Almacen::abrir(&r).unwrap();
        let id = a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        a.actualizar_pin(id, None).unwrap();

        let a2 = Almacen::abrir(&r).unwrap();
        let e = a2.entradas().iter().find(|e| e.id == id).unwrap();
        assert_eq!(e.pin, None, "cerrado");
        assert!(a2.ruta_objeto(e).is_file(), "el contenido sigue ahi");
    }

    #[test]
    fn los_ids_y_los_objetos_nunca_se_reutilizan() {
        // Caso negativo del contador: dos guardados dan ids y rutas
        // distintos aunque el primero se "cierre" entre medias.
        let r = raiz("contador");
        let mut a = Almacen::abrir(&r).unwrap();
        let id1 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        a.actualizar_pin(id1, None).unwrap();
        let id2 = a.guardar_imagen(BYTES, "recorte", None).unwrap();
        assert_ne!(id1, id2);
        let e1 = a
            .entradas()
            .iter()
            .find(|e| e.id == id1)
            .unwrap()
            .objeto
            .clone();
        let e2 = a
            .entradas()
            .iter()
            .find(|e| e.id == id2)
            .unwrap()
            .objeto
            .clone();
        assert_ne!(e1, e2, "cada objeto tiene su fichero");
    }

    #[test]
    fn actualizar_un_id_inexistente_da_error() {
        let r = raiz("no-existe");
        let mut a = Almacen::abrir(&r).unwrap();
        assert!(matches!(
            a.actualizar_pin(999, None),
            Err(ErrorAlmacen::NoExiste(999))
        ));
    }

    #[test]
    fn un_indice_con_claves_desconocidas_abre_igual() {
        // La regla de compatibilidad de los ajustes, aplicada al indice: un
        // fichero escrito por una version futura no impide arrancar.
        let r = raiz("futuro");
        {
            let mut a = Almacen::abrir(&r).unwrap();
            a.guardar_imagen(BYTES, "recorte", None).unwrap();
        }
        let ruta = r.join("almacen").join("indice.json");
        let texto = fs::read_to_string(&ruta).unwrap().replacen(
            "\"version\":",
            "\"funcion_del_futuro\": 42, \"version\":",
            1,
        );
        fs::write(&ruta, texto).unwrap();
        assert!(Almacen::abrir(&r).is_ok());
    }

    #[test]
    fn el_indice_se_escribe_por_temporal_mas_rename() {
        // No queda ningun .tmp tras operar: si quedara, la escritura no fue
        // atomica o el rename fallo en silencio.
        let r = raiz("atomico");
        let mut a = Almacen::abrir(&r).unwrap();
        a.guardar_imagen(BYTES, "recorte", Some(pin())).unwrap();
        let sobras: Vec<_> = fs::read_dir(r.join("almacen"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(sobras.is_empty(), "quedo un temporal: {sobras:?}");
    }
}
