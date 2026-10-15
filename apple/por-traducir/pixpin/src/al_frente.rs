//! **Lo que el usuario tiene abierto ahora**, para meterle lo que llega de
//! otro aparato del grupo (`suelto`, ver `pixpin_sincro::al_lienzo`).
//!
//! El usuario, 7-oct-2026: «pasar archivos de todo tipo de formato
//! rapidamente a cualquier chat que este abierto ese momento [...] en el
//! caso del canvas solo recibe fotos pero el chat cualquier cosa», y con un
//! lienzo delante y algo que no es foto, «directamente que se niegue».
//!
//! Lo apuntan las propias ventanas en su bucle, cada vez que estan delante:
//! el editor del lienzo ([`poner_lienzo`]) y la del chat ([`poner_chat`],
//! con el proyecto que ensena, o sin el si esta en la lista). Gana la ultima
//! que estuvo delante y sigue viva: quien manda desde otro aparato no esta
//! mirando este, asi que «abierto» es lo ultimo que se miro aqui. Es el
//! `LienzoAlFrente` de Android, con el chat al lado.
//!
//! Lo que va a un lienzo se le deja en su cola ([`entregar_al_lienzo`]) y se
//! le da un toque: solo su hilo puede tocar su escena. Lo que va a un chat
//! se escribe en el proyecto desde aqui mismo, como un adjunto, y la ventana
//! se refresca (`ventana_chat::refrescar`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pixpin_sincro::al_lienzo::{Donde, Llegada, Recibe, SOLO_FOTOS, Suelto};

/// Lo ultimo que estuvo delante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frente {
    Lienzo {
        hwnd: isize,
    },
    /// La ventana del chat; `proyecto` es `(id, nombre)` del que ensena, o
    /// `None` si esta en la lista de proyectos.
    Chat {
        hwnd: isize,
        proyecto: Option<(String, String)>,
    },
}

impl Frente {
    fn hwnd(&self) -> isize {
        match self {
            Frente::Lienzo { hwnd } | Frente::Chat { hwnd, .. } => *hwnd,
        }
    }
}

static FRENTE: Mutex<Option<Frente>> = Mutex::new(None);
/// Lo que espera a que su lienzo lo recoja: `(hwnd, fichero)`.
static PARA_LIENZOS: Mutex<Vec<(isize, PathBuf)>> = Mutex::new(Vec::new());

fn poner(f: Frente) {
    if let Ok(mut g) = FRENTE.lock()
        && g.as_ref() != Some(&f)
    {
        *g = Some(f);
    }
}

/// El editor de un lienzo esta delante.
pub fn poner_lienzo(hwnd: isize) {
    poner(Frente::Lienzo { hwnd });
}

/// La ventana del chat esta delante, con ese proyecto abierto (o ninguno).
pub fn poner_chat(hwnd: isize, proyecto: Option<(&str, &str)>) {
    poner(Frente::Chat {
        hwnd,
        proyecto: proyecto.map(|(i, n)| (i.to_string(), n.to_string())),
    });
}

/// Lo que esta abierto ahora, si sigue vivo.
pub fn actual() -> Option<Frente> {
    let f = FRENTE.lock().ok()?.clone()?;
    pixpin_shell::overlay::existe(f.hwnd()).then_some(f)
}

/// Deja `fichero` (una imagen) para el lienzo `hwnd` y lo despierta.
fn entregar_al_lienzo(hwnd: isize, fichero: PathBuf) {
    if let Ok(mut v) = PARA_LIENZOS.lock() {
        v.push((hwnd, fichero));
    }
    pixpin_shell::overlay::despertar(hwnd);
}

/// Lo que ha llegado para el lienzo `hwnd`. Lo llama su bucle en cada vuelta.
pub fn tomar_para(hwnd: isize) -> Vec<PathBuf> {
    let Ok(mut v) = PARA_LIENZOS.lock() else {
        return Vec::new();
    };
    if v.is_empty() {
        return Vec::new();
    }
    let (mias, otras): (Vec<_>, Vec<_>) = std::mem::take(&mut *v)
        .into_iter()
        .partition(|(h, _)| *h == hwnd);
    *v = otras;
    mias.into_iter().map(|(_, r)| r).collect()
}

/// A donde va algo que llega, segun lo que hay abierto. Pura, para probarla.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Destino {
    Lienzo(isize),
    Chat {
        id: String,
        nombre: String,
    },
    /// Nada abierto: «Mensajes guardados».
    Guardados,
}

fn decidir(frente: Option<&Frente>, es_imagen: bool) -> Result<Destino, ()> {
    match frente {
        Some(Frente::Lienzo { hwnd }) if es_imagen => Ok(Destino::Lienzo(*hwnd)),
        Some(Frente::Lienzo { .. }) => Err(()),
        Some(Frente::Chat {
            proyecto: Some((id, nombre)),
            ..
        }) => Ok(Destino::Chat {
            id: id.clone(),
            nombre: nombre.clone(),
        }),
        _ => Ok(Destino::Guardados),
    }
}

/// Quien atiende un `suelto` en este PC: lo pide el `Respondedor` de
/// `sincronizar`, uno por archivo.
pub struct AlFrente {
    pub raiz: PathBuf,
    /// Este equipo (`yo.nombre`), para el «solo acepta fotos».
    pub yo: String,
    /// Quien manda: su `id` sella los mensajes que entran en un chat.
    pub de: String,
    /// Donde quedan las fotos que esperan a su lienzo: el fichero recibido
    /// se borra al volver de `guardar`.
    pub carpeta_lienzo: PathBuf,
}

impl Recibe for AlFrente {
    fn aceptar(&mut self, p: &Suelto) -> Result<(), String> {
        decidir(actual().as_ref(), p.es_imagen())
            .map(|_| ())
            .map_err(|_| format!("{} {SOLO_FOTOS}", self.yo))
    }

    fn guardar(&mut self, p: &Suelto, fichero: &Path) -> Result<Llegada, String> {
        // Se decide otra vez: entre el «vale» y el ultimo trozo pudo cerrarse
        // el lienzo o cambiar el chat.
        let destino = decidir(actual().as_ref(), p.es_imagen())
            .map_err(|_| format!("{} {SOLO_FOTOS}", self.yo))?;
        let nombre = Path::new(&p.nombre)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| "archivo".into());
        match destino {
            Destino::Lienzo(hwnd) => {
                std::fs::create_dir_all(&self.carpeta_lienzo).map_err(|e| e.to_string())?;
                let propia = self.carpeta_lienzo.join(format!(
                    "{}-{nombre}",
                    crate::sincronizar::ahora_para_nombres()
                ));
                std::fs::copy(fichero, &propia).map_err(|e| e.to_string())?;
                entregar_al_lienzo(hwnd, propia);
                Ok(Llegada::en(Donde::Lienzo))
            }
            Destino::Chat { id, nombre: chat } => {
                meter_en_chat(&self.raiz, &id, &nombre, fichero, &self.de)?;
                Ok(Llegada {
                    donde: Donde::ChatAbierto,
                    chat: Some(chat),
                })
            }
            Destino::Guardados => {
                let ficha = pixpin_proyecto::almacen::asegurar_guardados(
                    &self.raiz,
                    crate::sincronizar::ahora_para_nombres(),
                    &self.de,
                )
                .map_err(|e| e.to_string())?;
                meter_en_chat(&self.raiz, &ficha.id, &nombre, fichero, &self.de)?;
                Ok(Llegada::en(Donde::Chat))
            }
        }
    }
}

/// Un adjunto nuevo en el chat `proyecto`, como si se hubiera soltado ahi.
fn meter_en_chat(
    raiz: &Path,
    proyecto: &str,
    nombre: &str,
    fichero: &Path,
    de: &str,
) -> Result<(), String> {
    let bytes = std::fs::read(fichero).map_err(|e| e.to_string())?;
    crate::ventana_chat::meter_en_proyecto(raiz, proyecto, &[(nombre.to_string(), bytes)], de)
        .map_err(|e| e.to_string())
        .and_then(|hechos| {
            if hechos.is_empty() {
                Err("No se pudo guardar en el chat".into())
            } else {
                Ok(())
            }
        })?;
    crate::ventana_chat::refrescar();
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn chat(p: Option<(&str, &str)>) -> Frente {
        Frente::Chat {
            hwnd: 1,
            proyecto: p.map(|(i, n)| (i.into(), n.into())),
        }
    }

    #[test]
    fn un_chat_abierto_recibe_cualquier_cosa() {
        let f = chat(Some(("p1", "Tesis")));
        let esperado = Destino::Chat {
            id: "p1".into(),
            nombre: "Tesis".into(),
        };
        assert_eq!(decidir(Some(&f), true), Ok(esperado.clone()));
        assert_eq!(decidir(Some(&f), false), Ok(esperado));
    }

    #[test]
    fn un_lienzo_abierto_recibe_fotos_y_niega_lo_demas() {
        let f = Frente::Lienzo { hwnd: 7 };
        assert_eq!(decidir(Some(&f), true), Ok(Destino::Lienzo(7)));
        // Caso negativo: un PDF a un lienzo se niega, no va a otro sitio.
        assert_eq!(decidir(Some(&f), false), Err(()));
    }

    #[test]
    fn sin_nada_abierto_o_en_la_lista_va_a_guardados() {
        assert_eq!(decidir(None, false), Ok(Destino::Guardados));
        assert_eq!(decidir(Some(&chat(None)), true), Ok(Destino::Guardados));
    }

    /// De punta a punta, por TCP y con el `Respondedor` de verdad: otro
    /// aparato del grupo manda un PDF a este PC. Sin nada abierto (en las
    /// pruebas no hay ventanas), entra como adjunto en «Mensajes guardados».
    #[test]
    fn un_pc_del_grupo_recibe_un_pdf_y_sin_nada_abierto_va_a_guardados() {
        use pixpin_proyecto::identidad::{Aparato, Identidad};
        use pixpin_sincro::mensajes as m;
        const CODIGO: &str = "ABCD2345";
        let raiz = std::env::temp_dir().join(format!("pixpin-al-frente-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let ficha = |id: &str, nombre: &str, letra: &str| Aparato {
            id: id.into(),
            nombre: nombre.into(),
            letra: Some(letra.into()),
            ..Default::default()
        };
        Identidad {
            yo: ficha("pc", "Portatil", "A"),
            codigo: Some(CODIGO.into()),
            miembros: vec![ficha("pc", "Portatil", "A"), ficha("tel", "Telefono", "B")],
            ..Default::default()
        }
        .guardar(&raiz)
        .unwrap();
        let escucha = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let r2 = raiz.clone();
        let hilo = std::thread::spawn(move || {
            let (flujo, _) = escucha.accept().unwrap();
            let disco = pixpin_proyecto::vista::DiscoPc::nuevo(&r2);
            let fabrica = |otro: &m::Aparato| -> Box<dyn Recibe> {
                Box::new(AlFrente {
                    raiz: r2.clone(),
                    yo: "Portatil".into(),
                    de: otro.id.clone(),
                    carpeta_lienzo: r2.join("al-lienzo"),
                })
            };
            pixpin_sincro::protocolo::Respondedor {
                disco: &disco,
                estado: &|_| {},
                ahora: &|| 1,
                mi_puerto: 0,
                al_saludar: &|_, _| {},
                suelto: Some(&fabrica),
            }
            .atender(flujo, [3; 32])
        });
        let yo = m::Aparato {
            id: "tel".into(),
            nombre: "Telefono".into(),
            letra: Some("B".into()),
            desde: 0,
        };
        let hola = m::Hola {
            yo: yo.clone(),
            miembros: vec![
                m::Aparato {
                    id: "pc".into(),
                    nombre: "Portatil".into(),
                    letra: Some("A".into()),
                    desde: 0,
                },
                yo,
            ],
            reloj: 1,
            ..Default::default()
        };
        let pdf = raiz.join("informe.pdf");
        std::fs::write(&pdf, b"%PDF-1.7 de prueba").unwrap();
        let flujo = std::net::TcpStream::connect(("127.0.0.1", puerto)).unwrap();
        let mut c =
            pixpin_sincro::al_lienzo::Conexion::abrir(flujo, CODIGO, [4; 32], hola).unwrap();
        let l = c.mandar(&pdf, "informe.pdf").unwrap();
        assert_eq!(l, Llegada::en(Donde::Chat));
        c.adios();
        hilo.join().unwrap().unwrap();
        // Esta en «Mensajes guardados», como un adjunto con su nombre.
        let indice = pixpin_proyecto::almacen::Indice::leer(&raiz);
        let guardados = indice
            .proyectos
            .iter()
            .find(|f| f.es_guardados())
            .expect("se creo Mensajes guardados");
        let cuaderno = pixpin_proyecto::cuaderno::Cuaderno::leer_de(
            &pixpin_proyecto::almacen::carpeta(&raiz, &guardados.id),
        )
        .unwrap();
        assert!(
            format!("{cuaderno:?}").contains("informe.pdf"),
            "el PDF no esta en el chat"
        );
    }

    #[test]
    fn cada_lienzo_recoge_solo_lo_suyo() {
        // Handles que no son ventanas: `despertar` falla y se ignora.
        entregar_al_lienzo(-11, PathBuf::from("a.png"));
        entregar_al_lienzo(-12, PathBuf::from("b.png"));
        entregar_al_lienzo(-11, PathBuf::from("c.png"));
        assert_eq!(
            tomar_para(-11),
            [PathBuf::from("a.png"), PathBuf::from("c.png")]
        );
        assert!(tomar_para(-11).is_empty());
        assert_eq!(tomar_para(-12), [PathBuf::from("b.png")]);
    }
}
