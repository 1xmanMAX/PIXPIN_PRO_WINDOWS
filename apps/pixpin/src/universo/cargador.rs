//! Leer los cuadernos sin parar la ventana (D236), y soltarlos cuando se
//! alejan.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant, SystemTime};

use pixpin_universo::desde_el_chat::NodoDelChat;
use pixpin_universo::ficha::FichaLuna;

pub const SOLTAR_TRAS: Duration = Duration::from_secs(60);
pub const CARGADOS_MAXIMOS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sello {
    pub bytes: u64,
    pub modificado: SystemTime,
}

/// D243: fecha y tamano del cuaderno. Si no cambian, no se relee.
pub fn sello_de(carpeta: &Path) -> Option<Sello> {
    let m = std::fs::metadata(carpeta.join("guardados.jsonl")).ok()?;
    Some(Sello {
        bytes: m.len(),
        modificado: m.modified().ok()?,
    })
}

#[derive(Debug)]
pub struct Cargado {
    pub proyecto: String,
    pub fichas: Vec<FichaLuna>,
    /// La forma del chat, para armar la galaxia con ella (H2). Solo los
    /// mensajes: las hojas que no estan en el chat van a la nebulosa, y de
    /// alli se colocan a mano (H3), como el «Anadir > Hoja» del movil.
    pub nodos: Vec<NodoDelChat>,
    pub rotas: usize,
    pub sello: Option<Sello>,
    pub error: Option<String>,
}

struct Galaxia {
    fichas: Vec<FichaLuna>,
    sello: Option<Sello>,
    usado: Instant,
}

pub struct Cargador {
    raiz: PathBuf,
    aviso: Option<isize>,
    enviar: Sender<Cargado>,
    llegar: Receiver<Cargado>,
    pedidos: HashSet<String>,
    galaxias: HashMap<String, Galaxia>,
}

/// Que proyectos soltar: con mas de `CARGADOS_MAXIMOS`, los lejanos sin
/// usar desde hace `SOLTAR_TRAS`, del mas antiguo al mas nuevo, hasta
/// quedar en el maximo. Pura, para probarla.
pub fn politica_soltar(
    usos: &[(String, Instant)],
    ahora: Instant,
    lejos: &HashSet<String>,
) -> Vec<String> {
    if usos.len() <= CARGADOS_MAXIMOS {
        return Vec::new();
    }
    let mut candidatos: Vec<&(String, Instant)> = usos
        .iter()
        .filter(|(p, t)| lejos.contains(p) && ahora.duration_since(*t) >= SOLTAR_TRAS)
        .collect();
    candidatos.sort_by_key(|(_, t)| *t);
    candidatos
        .into_iter()
        .take(usos.len() - CARGADOS_MAXIMOS)
        .map(|(p, _)| p.clone())
        .collect()
}

/// **Las hojas del proyecto que no estan en el chat** (H3): las paginas de
/// un PDF sin dibujo, los lienzos del movil. Son las mismas que el chat y la
/// galeria ensenan sin escribirlas en el cuaderno
/// (`almacen::hojas_para_ensenar`), y aqui entran como fichas: salen en la
/// nebulosa y se colocan arrastrandolas, y al abrirlas van a su hoja por
/// `referencia` (D224). No se arman solas: el movil tampoco las pone, las
/// anade el usuario con «Anadir > Hoja».
fn hojas_sueltas(raiz: &Path, proyecto: &str) -> Vec<FichaLuna> {
    let aparato = pixpin_proyecto::almacen::Indice::leer(raiz)
        .buscar(proyecto)
        .and_then(|f| f.aparato.clone())
        .unwrap_or_default();
    pixpin_proyecto::almacen::hojas_para_ensenar(raiz, proyecto, &aparato)
        .iter()
        .map(|m| {
            let mut f = super::fichas::de_mensaje(m, raiz, proyecto);
            // Una hoja se abre en su lienzo aunque no tenga fichero propio
            // (una pagina sin dibujo va por su codigo, `abrir`): no es un
            // fantasma.
            f.en_equipo = true;
            f
        })
        .collect()
}

impl Cargador {
    /// `aviso`: el HWND de la ventana a despertar (`MSG_DESPIERTA`) cuando
    /// llega algo. `None` en las pruebas.
    pub fn nuevo(raiz: PathBuf, aviso: Option<isize>) -> Self {
        let (enviar, llegar) = channel();
        Self {
            raiz,
            aviso,
            enviar,
            llegar,
            pedidos: HashSet::new(),
            galaxias: HashMap::new(),
        }
    }

    /// La ventana a despertar cuando llegue un cuaderno. Se pone al abrirla:
    /// el cargador nace antes que la ventana.
    pub fn poner_aviso(&mut self, hwnd: isize) {
        self.aviso = Some(hwnd);
    }

    /// Si hay lecturas en marcha: mientras las haya, el universo espera a
    /// que lo despierten, no hace falta sondear.
    pub fn leyendo(&self) -> bool {
        !self.pedidos.is_empty()
    }

    /// Lee el cuaderno de `proyecto` en otro hilo: TODO el chat, notas
    /// incluidas, porque desde H2 la galaxia se arma con la forma del chat
    /// entero (un comentario es un cuerpo pequeno en orbita). Lo que el
    /// interruptor «notas del chat» decide ahora es solo si las notas sin
    /// colocar salen en la nebulosa (`Sesion::sueltas`).
    pub fn pedir(&mut self, proyecto: &str) {
        if !self.pedidos.insert(proyecto.to_string()) {
            return;
        }
        let (raiz, p, tx, aviso) = (
            self.raiz.clone(),
            proyecto.to_string(),
            self.enviar.clone(),
            self.aviso,
        );
        let lanzado = std::thread::Builder::new()
            .name("universo-cuaderno".into())
            .spawn(move || {
                let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, &p);
                let sello = sello_de(&carpeta);
                let c = match pixpin_proyecto::cuaderno::Cuaderno::leer_de(&carpeta) {
                    Ok(c) => {
                        let mut fichas = super::fichas::de_cuaderno(&c, &raiz, &p, true);
                        fichas.extend(hojas_sueltas(&raiz, &p));
                        Cargado {
                            fichas,
                            nodos: super::fichas::nodos_de(&c),
                            rotas: c.lineas_rotas,
                            sello,
                            error: None,
                            proyecto: p,
                        }
                    }
                    // Un proyecto sin cuaderno todavia es un proyecto vacio
                    // de chat, pero puede tener hojas.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Cargado {
                        fichas: hojas_sueltas(&raiz, &p),
                        nodos: Vec::new(),
                        rotas: 0,
                        sello,
                        error: None,
                        proyecto: p,
                    },
                    Err(e) => Cargado {
                        fichas: Vec::new(),
                        nodos: Vec::new(),
                        rotas: 0,
                        sello,
                        error: Some(e.to_string()),
                        proyecto: p,
                    },
                };
                let _ = tx.send(c);
                if let Some(h) = aviso {
                    pixpin_shell::overlay::despertar(h);
                }
            });
        if let Err(e) = lanzado {
            tracing::warn!(?e, "no se pudo lanzar la lectura del cuaderno");
            self.pedidos.remove(proyecto);
        }
    }

    /// Lo que llego desde la ultima vez. Llamar con `EventoOverlay::Despierta`.
    pub fn recibir(&mut self) -> Vec<Cargado> {
        let mut llegados = Vec::new();
        while let Ok(c) = self.llegar.try_recv() {
            self.pedidos.remove(&c.proyecto);
            if c.error.is_none() {
                self.galaxias.insert(
                    c.proyecto.clone(),
                    Galaxia {
                        fichas: c.fichas.clone(),
                        sello: c.sello,
                        usado: Instant::now(),
                    },
                );
            }
            llegados.push(c);
        }
        llegados
    }

    pub fn cargado(&self, proyecto: &str) -> Option<&[FichaLuna]> {
        self.galaxias.get(proyecto).map(|g| g.fichas.as_slice())
    }

    pub fn tocar(&mut self, proyecto: &str, ahora: Instant) {
        if let Some(g) = self.galaxias.get_mut(proyecto) {
            g.usado = ahora;
        }
    }

    pub fn a_soltar(&self, ahora: Instant, lejos: &HashSet<String>) -> Vec<String> {
        let usos: Vec<(String, Instant)> = self
            .galaxias
            .iter()
            .map(|(p, g)| (p.clone(), g.usado))
            .collect();
        politica_soltar(&usos, ahora, lejos)
    }

    pub fn soltar(&mut self, proyecto: &str) {
        self.galaxias.remove(proyecto);
    }

    /// D243: los cargados cuyo cuaderno cambio en el disco.
    pub fn cambiados(&self) -> Vec<String> {
        self.galaxias
            .iter()
            .filter(|(p, g)| sello_de(&pixpin_proyecto::almacen::carpeta(&self.raiz, p)) != g.sello)
            .map(|(p, _)| p.clone())
            .collect()
    }

    pub fn todas(&self) -> impl Iterator<Item = &FichaLuna> {
        self.galaxias.values().flat_map(|g| g.fichas.iter())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn se_suelta_el_mas_antiguo_lejano_solo_con_mas_de_cuatro_y_tras_sesenta_segundos() {
        let t0 = Instant::now();
        let mut usos = vec![];
        for (i, p) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            usos.push((p.to_string(), t0 + Duration::from_secs(i as u64)));
        }
        let lejos: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        // Aun no pasaron 60 s.
        assert!(politica_soltar(&usos, t0 + Duration::from_secs(30), &lejos).is_empty());
        // Pasaron: sale solo el mas antiguo de los lejanos, y solo lo que sobra de 4.
        assert_eq!(
            politica_soltar(&usos, t0 + Duration::from_secs(120), &lejos),
            vec!["a".to_string()]
        );
        // Con 4 cargados no se suelta nada.
        assert!(politica_soltar(&usos[..4], t0 + Duration::from_secs(120), &lejos).is_empty());
    }

    #[test]
    fn leer_un_cuaderno_llega_por_el_canal_con_sus_fichas() {
        let raiz = std::env::temp_dir().join(format!("pixpin-cargador-{}", std::process::id()));
        let carpeta = pixpin_proyecto::almacen::carpeta(&raiz, "p1");
        std::fs::create_dir_all(&carpeta).unwrap();
        std::fs::write(
            carpeta.join("guardados.jsonl"),
            "{\"id\":\"1\",\"clase\":\"ARCHIVO\",\"nombre\":\"a.txt\"}\nroto\n",
        )
        .unwrap();
        let mut c = Cargador::nuevo(raiz.clone(), None);
        c.pedir("p1");
        let mut llegados = Vec::new();
        for _ in 0..200 {
            llegados.extend(c.recibir());
            if !llegados.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(llegados.len(), 1);
        assert_eq!(llegados[0].fichas.len(), 1);
        assert_eq!(llegados[0].rotas, 1);
        assert!(c.cargado("p1").is_some());
    }
}
