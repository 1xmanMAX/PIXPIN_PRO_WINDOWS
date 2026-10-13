//! **Fotos y audios de una leccion** (`AdjuntosDeLaLeccion.kt`, v0.102):
//! la foto del error en la obra, la nota de voz con lo que dijo el profesor.
//! Dos botones —galeria y grabar— y la tira de lo puesto, cada cosa con su
//! aspa. Vale para una leccion nueva **y para una ya guardada** (antes, en
//! el PC, las fotos solo iban con una nueva).
//!
//! Lo nuevo son [`Adjuntos::fotos`] y [`Adjuntos::audios`]: aun sin chat, se
//! meten en el al guardar la leccion (`almacen::guardar_con_cambios`), como
//! mensajes que **le responden**. Lo que ya estaba son
//! [`Adjuntos::viejos`]; quitarlo lo apunta en [`Adjuntos::quitados`] y al
//! guardar sale del chat con su marca de borrado (viaja: el movil tambien lo
//! quita). Descartar la ficha no toca nada.
//!
//! Lo que cambia por ser Windows: no hay camara; «Galeria» es el dialogo
//! «Abrir» de fotos. Grabar usa el microfono como las notas de voz del chat.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use pixpin_proyecto::cuaderno::{Clase, Mensaje};

use super::almacen;

/// Lo que se ve en la tira, en orden: lo viejo que queda y lo nuevo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visible {
    /// Un mensaje que ya estaba (indice en `viejos`).
    Viejo(usize),
    /// Una foto nueva (indice en `fotos`).
    Foto(usize),
    /// Un audio nuevo (indice en `audios`).
    Audio(usize),
}

/// Grabar aqui mismo, sin salir de la ficha.
pub enum Grabar {
    Nada,
    Abriendo(Receiver<Result<pixpin_audio::Grabadora, pixpin_audio::ErrorAudio>>),
    Grabando(pixpin_audio::Grabadora, Instant),
    Parando(Receiver<Result<pixpin_audio::Grabacion, pixpin_audio::ErrorAudio>>),
}

/// Lo que se guarda con la leccion.
pub struct Adjuntos {
    pub viejos: Vec<Mensaje>,
    /// Donde esta en disco cada viejo (para su miniatura y para abrirlo).
    pub rutas_viejos: Vec<Option<PathBuf>>,
    pub quitados: Vec<String>,
    pub fotos: Vec<almacen::Foto>,
    /// De donde salio cada foto nueva (para su miniatura); las que llegan
    /// en memoria (Flow Launcher) no tienen.
    pub rutas_fotos: Vec<Option<PathBuf>>,
    pub audios: Vec<almacen::Voz>,
    pub grabar: Grabar,
}

/// Las fotos que se leen de la galeria: como mucho estas a la vez (las seis
/// del selector de Android).
pub const MAXIMO_DE_GALERIA: usize = 6;

impl Adjuntos {
    pub fn nuevos(fotos: Vec<almacen::Foto>) -> Self {
        let n = fotos.len();
        Self { viejos: Vec::new(), rutas_viejos: Vec::new(), quitados: Vec::new(), fotos, rutas_fotos: vec![None; n], audios: Vec::new(), grabar: Grabar::Nada }
    }

    /// Los de una leccion ya guardada.
    pub fn de(raiz: &Path, e: &almacen::Entrada) -> Self {
        let viejos = almacen::adjuntos_de(raiz, e);
        let rutas_viejos = viejos
            .iter()
            .map(|m| m.ruta.as_deref().and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &e.ficha, r)))
            .collect();
        Self { viejos, rutas_viejos, ..Self::nuevos(Vec::new()) }
    }

    /// La tira: lo viejo que no se quito, y lo nuevo.
    pub fn visibles(&self) -> Vec<Visible> {
        let mut v: Vec<Visible> = (0..self.viejos.len()).filter(|i| !self.quitados.contains(&self.viejos[*i].id)).map(Visible::Viejo).collect();
        v.extend((0..self.fotos.len()).map(Visible::Foto));
        v.extend((0..self.audios.len()).map(Visible::Audio));
        v
    }

    /// El aspa de la `i`-esima de la tira: lo nuevo se olvida; lo viejo se
    /// apunta para quitarlo al guardar.
    pub fn quitar(&mut self, i: usize) {
        match self.visibles().get(i) {
            Some(Visible::Viejo(k)) => {
                let id = self.viejos[*k].id.clone();
                if !self.quitados.contains(&id) {
                    self.quitados.push(id);
                }
            }
            Some(Visible::Foto(k)) => {
                self.fotos.remove(*k);
                self.rutas_fotos.remove(*k);
            }
            Some(Visible::Audio(k)) => {
                self.audios.remove(*k);
            }
            None => {}
        }
    }

    /// Añade fotos de disco (las de la galeria). Lo que no se lee o no es
    /// una imagen se salta. Devuelve cuantas.
    pub fn anadir_fotos(&mut self, rutas: &[PathBuf]) -> usize {
        let mut n = 0;
        for r in rutas.iter().take(MAXIMO_DE_GALERIA) {
            if !es_imagen(r) {
                continue;
            }
            let Ok(bytes) = std::fs::read(r) else { continue };
            let nombre = r.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "foto.jpg".into());
            self.fotos.push((nombre, bytes));
            self.rutas_fotos.push(Some(r.clone()));
            n += 1;
        }
        n
    }

    /// Las rutas con miniatura (para cargarlas antes de pintar).
    pub fn rutas_con_miniatura(&self) -> Vec<PathBuf> {
        self.visibles()
            .iter()
            .filter_map(|v| match v {
                Visible::Viejo(k) if self.viejos[*k].clase == Some(Clase::Imagen) => self.rutas_viejos[*k].clone(),
                Visible::Foto(k) => self.rutas_fotos[*k].clone(),
                _ => None,
            })
            .collect()
    }

    pub fn grabando(&self) -> bool {
        !matches!(self.grabar, Grabar::Nada)
    }

    /// El clic en «Audio»: empezar o parar.
    pub fn pulsar_audio(&mut self, temporal: PathBuf) {
        self.grabar = match std::mem::replace(&mut self.grabar, Grabar::Nada) {
            Grabar::Nada => Grabar::Abriendo(crate::ventanita::abrir_microfono(temporal)),
            Grabar::Grabando(g, _) => Grabar::Parando(crate::ventanita::cerrar_microfono(g)),
            otro => otro,
        };
    }

    /// Mira el microfono; devuelve un aviso si algo salio mal.
    pub fn avanzar(&mut self) -> Option<String> {
        let (sig, aviso) = match std::mem::replace(&mut self.grabar, Grabar::Nada) {
            Grabar::Nada => (Grabar::Nada, None),
            Grabar::Abriendo(rx) => match rx.try_recv() {
                Ok(Ok(g)) => (Grabar::Grabando(g, Instant::now()), None),
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => (Grabar::Nada, Some("No se pudo usar el micrófono".to_string())),
                Err(TryRecvError::Empty) => (Grabar::Abriendo(rx), None),
            },
            g @ Grabar::Grabando(..) => (g, None),
            Grabar::Parando(rx) => match rx.try_recv() {
                Ok(Ok(gr)) => {
                    let leido = std::fs::read(&gr.ruta);
                    let _ = std::fs::remove_file(&gr.ruta);
                    match leido {
                        Ok(b) if !b.is_empty() => {
                            let nombre = format!("voz_{}.m4a", pixpin_shell::entorno::ahora_utc_ms());
                            self.audios.push((nombre, b, gr.duracion_ms));
                            (Grabar::Nada, None)
                        }
                        _ => (Grabar::Nada, Some("Muy corto: no se guardó".to_string())),
                    }
                }
                Ok(Err(pixpin_audio::ErrorAudio::DemasiadoCorta { .. })) => (Grabar::Nada, Some("Muy corto: no se guardó".to_string())),
                Ok(Err(_)) | Err(TryRecvError::Disconnected) => (Grabar::Nada, Some("No se pudo guardar el audio".to_string())),
                Err(TryRecvError::Empty) => (Grabar::Parando(rx), None),
            },
        };
        self.grabar = sig;
        aviso
    }

    /// El rotulo del boton de grabar.
    pub fn rotulo_audio(&self) -> String {
        match &self.grabar {
            Grabar::Grabando(_, desde) => format!("⏹ Parar {}", duracion_legible(desde.elapsed().as_millis() as i64)),
            Grabar::Abriendo(_) => "🎙 Abriendo…".into(),
            Grabar::Parando(_) => "🎙 Guardando…".into(),
            Grabar::Nada => "🎙 Audio".into(),
        }
    }
}

/// «0:07», «12:30» (`duracionLegible` del movil).
pub fn duracion_legible(ms: i64) -> String {
    let s = (ms.max(0) + 500) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

fn es_imagen(r: &Path) -> bool {
    let e = r.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn mensaje(id: &str, clase: Clase) -> Mensaje {
        serde_json::from_value(serde_json::json!({ "id": id, "cuando": 1, "clase": match clase { Clase::Voz => "VOZ", _ => "IMAGEN" } })).unwrap()
    }

    #[test]
    fn quitar_lo_viejo_lo_apunta_y_quitar_lo_nuevo_lo_olvida() {
        let mut a = Adjuntos::nuevos(vec![("f.jpg".into(), vec![1])]);
        a.viejos = vec![mensaje("v1", Clase::Imagen), mensaje("v2", Clase::Voz)];
        a.rutas_viejos = vec![None, None];
        a.audios.push(("voz.m4a".into(), vec![2], 3_000));
        assert_eq!(a.visibles(), vec![Visible::Viejo(0), Visible::Viejo(1), Visible::Foto(0), Visible::Audio(0)]);
        a.quitar(1);
        assert_eq!(a.quitados, vec!["v2".to_string()]);
        assert_eq!(a.visibles(), vec![Visible::Viejo(0), Visible::Foto(0), Visible::Audio(0)]);
        // Ahora la foto nueva es la segunda de la tira.
        a.quitar(1);
        assert!(a.fotos.is_empty() && a.rutas_fotos.is_empty());
        assert_eq!(a.audios.len(), 1);
        a.quitar(1);
        assert!(a.audios.is_empty());
        // Caso negativo: un aspa fuera de la tira no hace nada.
        a.quitar(9);
        assert_eq!(a.visibles(), vec![Visible::Viejo(0)]);
        assert_eq!(a.quitados, vec!["v2".to_string()]);
    }

    #[test]
    fn de_la_galeria_solo_entran_imagenes_que_se_leen() {
        let d = std::env::temp_dir().join(format!("pixpin-leccion-galeria-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("obra.JPG"), b"jpg").unwrap();
        std::fs::write(d.join("notas.txt"), b"no").unwrap();
        let mut a = Adjuntos::nuevos(Vec::new());
        let n = a.anadir_fotos(&[d.join("obra.JPG"), d.join("notas.txt"), d.join("no-esta.png")]);
        assert_eq!(n, 1);
        assert_eq!(a.fotos[0].0, "obra.JPG");
        assert_eq!(a.rutas_con_miniatura(), vec![d.join("obra.JPG")]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_duracion_se_lee_como_en_el_movil() {
        assert_eq!(duracion_legible(0), "0:00");
        assert_eq!(duracion_legible(7_400), "0:07");
        assert_eq!(duracion_legible(750_000), "12:30");
        assert_eq!(duracion_legible(-5), "0:00");
    }
}
