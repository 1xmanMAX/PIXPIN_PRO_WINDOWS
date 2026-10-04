//! **Grupos de ventanas guardados** (H9): lo de la multitarea del movil que
//! si encaja en un escritorio.
//!
//! En el movil (`data/LienzosAbiertos.kt`, `GrupoDeAbiertos`) lo abierto en
//! la multitarea se guarda con un nombre, desde la baraja, y se vuelve a
//! abrir de un toque desde el menu de inicio. Aqui la multitarea ya la da
//! Windows (ventanas, Snap, escritorios, Alt+Tab: H10), asi que lo que se
//! guarda es **que ventanas de PixPin habia y donde estaban**: por cada una,
//! que es (un lienzo de un proyecto, un documento en su lector, una nota
//! Markdown) y su `WINDOWPLACEMENT`. Reabrir el grupo abre lo que falte y
//! devuelve cada ventana a su sitio con `SetWindowPlacement`
//! (`pixpin_shell::colocacion`).
//!
//! # Como se sabe que hay abierto
//!
//! Cada ventana que se puede guardar se **apunta** al nacer ([`apuntar`]):
//! una linea en quien la abre, que deja su clase y su hilo en una lista y se
//! borra sola al cerrarse (es un guardian). La ventana se encuentra despues
//! por su hilo (`EnumThreadWindows`), sin tocar el codigo que la crea: el
//! lector y el lienzo no saben nada de grupos. Las ventanas que el hilo ya
//! tenia al apuntarse no cuentan: asi el lienzo, que se abre en el hilo del
//! chat, no se confunde con el chat.
//!
//! # Sin atajo global
//!
//! Se ofrece donde el movil: guardar y abrir en un menu (los tres puntos del
//! chat, que es la ventana principal, y la bandeja). Nada de teclas nuevas.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use pixpin_shell::colocacion::{self, Colocacion};
use pixpin_store::{Catalogo, Ubicacion};

/// Que es una ventana, con lo justo para volver a abrirla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "clase", rename_all = "lowercase")]
pub enum Clase {
    /// Una hoja dibujada de un proyecto: su proyecto y la referencia de su
    /// dibujo (o el codigo unico de la pagina que aun no lo tiene), que es
    /// como la busca el universo (`abrir_hoja::abrir_hoja`).
    Lienzo {
        proyecto: String,
        referencia: String,
    },
    /// Un documento en su lector: PDF, Word, libro o pagina.
    Lector { ruta: PathBuf },
    /// Una nota Markdown en su editor.
    Nota { destino: crate::notas_md::Destino },
}

/// Una ventana del grupo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ventana {
    pub clase: Clase,
    pub colocacion: Colocacion,
}

/// Un grupo guardado: lo abierto en un momento, con nombre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grupo {
    pub id: String,
    pub nombre: String,
    pub ventanas: Vec<Ventana>,
    /// Milisegundos desde 1970.
    pub guardado: i64,
}

/// Cuantos grupos se guardan; pasados estos, el mas viejo se va
/// (`Abiertos.GRUPOS` del movil).
pub const GRUPOS: usize = 12;

/// Guarda `ventanas` como grupo: uno con el mismo nombre se sustituye y el
/// nuevo va primero. Un nombre en blanco se llama «Grupo N». Es
/// `Abiertos.conGrupo` del movil tal cual.
pub fn con_grupo(
    grupos: &[Grupo],
    nombre: &str,
    ventanas: Vec<Ventana>,
    ahora: i64,
    sin_nombre: &str,
) -> Vec<Grupo> {
    let limpio = match nombre.trim() {
        "" => format!("{sin_nombre} {}", grupos.len() + 1),
        n => n.to_string(),
    };
    let nuevo = Grupo {
        id: format!("g-{ahora}"),
        nombre: limpio.clone(),
        ventanas: sin_repetir(ventanas),
        guardado: ahora,
    };
    std::iter::once(nuevo)
        .chain(
            grupos
                .iter()
                .filter(|g| g.nombre.to_lowercase() != limpio.to_lowercase())
                .cloned(),
        )
        .take(GRUPOS)
        .collect()
}

/// La misma cosa abierta dos veces (dos lectores del mismo PDF) se guarda
/// una: reabrir el grupo no tiene por que duplicarla.
fn sin_repetir(ventanas: Vec<Ventana>) -> Vec<Ventana> {
    let mut v: Vec<Ventana> = Vec::with_capacity(ventanas.len());
    for w in ventanas {
        if !v.iter().any(|x| x.clase == w.clase) {
            v.push(w);
        }
    }
    v
}

pub fn sin_grupo(grupos: &[Grupo], id: &str) -> Vec<Grupo> {
    grupos.iter().filter(|g| g.id != id).cloned().collect()
}

/// Donde se guardan: en la raiz de los datos y **no** en `proyectos/`. Son de
/// este equipo, como en el movil («es de este aparato: no viaja»): la
/// colocacion de una ventana no significa nada en otra pantalla.
fn fichero(raiz: &Path) -> PathBuf {
    raiz.join("grupos-de-ventanas.json")
}

pub fn leer(raiz: &Path) -> Vec<Grupo> {
    std::fs::read_to_string(fichero(raiz))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn escribir(raiz: &Path, grupos: &[Grupo]) -> std::io::Result<()> {
    let texto = serde_json::to_string_pretty(grupos).map_err(std::io::Error::other)?;
    // A un temporal y luego se cambia el nombre: un corte a media escritura
    // deja los grupos de antes enteros.
    let destino = fichero(raiz);
    let temporal = destino.with_extension("json.tmp");
    std::fs::write(&temporal, texto)?;
    std::fs::rename(&temporal, &destino)
}

// ---------------------------------------------------------------------------
// Lo abierto ahora

struct Registro {
    n: u64,
    hilo: u32,
    clase: Clase,
    /// La ventana, si quien se apunto la dio; si no, se busca por el hilo.
    hwnd: Option<isize>,
    /// Las ventanas que el hilo ya tenia al apuntarse: esas no son esta.
    antes: Vec<isize>,
}

static ABIERTAS: Mutex<Vec<Registro>> = Mutex::new(Vec::new());
static SIGUIENTE: AtomicU64 = AtomicU64::new(1);

/// El apunte de una ventana abierta. Mientras viva, la ventana cuenta como
/// abierta; al soltarse (al cerrarse la ventana) deja de contar.
pub struct Apunte {
    n: u64,
}

/// Apunta que el hilo que llama va a abrir (o ya abrio) una ventana de esa
/// clase. Hay que guardar lo que devuelve hasta que la ventana se cierre.
pub fn apuntar(clase: Clase) -> Apunte {
    let hilo = colocacion::hilo_actual();
    let n = SIGUIENTE.fetch_add(1, Ordering::SeqCst);
    let antes = colocacion::ventanas_del_hilo(hilo);
    if let Ok(mut v) = ABIERTAS.lock() {
        v.push(Registro {
            n,
            hilo,
            clase,
            hwnd: None,
            antes,
        });
    }
    Apunte { n }
}

impl Apunte {
    /// Dice cual es la ventana, cuando quien se apunto la conoce.
    pub fn con_ventana(&self, hwnd: isize) {
        self.cambiar(|r| r.hwnd = Some(hwnd));
    }

    /// Lo abierto cambio de identidad: una nota nueva que, al guardarse,
    /// pasa a ser un mensaje del proyecto.
    pub fn cambiar_clase(&self, clase: Clase) {
        self.cambiar(|r| r.clase = clase.clone());
    }

    fn cambiar(&self, f: impl Fn(&mut Registro)) {
        if let Ok(mut v) = ABIERTAS.lock()
            && let Some(r) = v.iter_mut().find(|r| r.n == self.n)
        {
            f(r);
        }
    }
}

impl Drop for Apunte {
    fn drop(&mut self) {
        if let Ok(mut v) = ABIERTAS.lock() {
            v.retain(|r| r.n != self.n);
        }
    }
}

fn ventana_de(r: &Registro) -> Option<isize> {
    if let Some(h) = r.hwnd {
        return colocacion::es_principal(h).then_some(h);
    }
    colocacion::ventanas_del_hilo(r.hilo)
        .into_iter()
        .find(|h| !r.antes.contains(h) && colocacion::es_principal(*h))
}

/// Lo que hay abierto y se ve, con su ventana.
fn abiertas() -> Vec<(Clase, isize)> {
    let registros: Vec<(Clase, Option<isize>)> = match ABIERTAS.lock() {
        Ok(v) => v.iter().map(|r| (r.clase.clone(), ventana_de(r))).collect(),
        Err(_) => Vec::new(),
    };
    registros
        .into_iter()
        .filter_map(|(c, h)| Some((c, h?)))
        .collect()
}

fn esta_abierta(clase: &Clase) -> bool {
    abiertas().iter().any(|(c, _)| c == clase)
}

/// Lo abierto ahora, cada cosa con su colocacion.
pub fn fotografiar() -> Vec<Ventana> {
    abiertas()
        .into_iter()
        .filter_map(|(clase, h)| {
            Some(Ventana {
                clase,
                colocacion: colocacion::leer(h)?,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Devolver cada ventana a su sitio

/// Lo que se le va a aplicar a una ventana que aun no ha nacido, o que ya
/// estaba abierta. Con su hora, para no esperar para siempre a una ventana
/// que no llega (un PDF que ya no existe).
static PENDIENTES: Mutex<Vec<(Clase, Colocacion, Instant)>> = Mutex::new(Vec::new());
static VIGILANDO: AtomicBool = AtomicBool::new(false);
/// Cuanto se espera a que nazca una ventana del grupo. Un lector de PDF
/// grande tarda en abrir en el equipo suelo; pasado esto, se deja.
const ESPERA: Duration = Duration::from_secs(20);

/// La colocacion que espera una ventana de esa clase, si la hay. La toma el
/// editor de notas antes de ensenarse, y asi nace ya en su sitio; las demas
/// ventanas las coloca el vigia al verlas aparecer.
pub fn tomar_colocacion(clase: &Clase) -> Option<Colocacion> {
    let mut v = PENDIENTES.lock().ok()?;
    let i = v.iter().position(|(c, _, _)| c == clase)?;
    Some(v.remove(i).1)
}

/// Coloca en su hilo lo que va apareciendo, y se va cuando no queda nada.
fn vigilar() {
    if VIGILANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    let lanzado = std::thread::Builder::new()
        .name("grupos-ventanas".into())
        .spawn(|| {
            loop {
                let listos: Vec<(isize, Colocacion)> = {
                    let Ok(mut v) = PENDIENTES.lock() else { break };
                    v.retain(|(_, _, desde)| desde.elapsed() < ESPERA);
                    let hay = abiertas();
                    let mut listos = Vec::new();
                    v.retain(|(clase, c, _)| match hay.iter().find(|(k, _)| k == clase) {
                        Some((_, h)) => {
                            listos.push((*h, *c));
                            false
                        }
                        None => true,
                    });
                    if v.is_empty() && listos.is_empty() {
                        break;
                    }
                    listos
                };
                for (h, c) in listos {
                    if !colocacion::poner(h, &c) {
                        tracing::warn!("no se pudo devolver una ventana a su sitio");
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            VIGILANDO.store(false, Ordering::SeqCst);
        });
    if lanzado.is_err() {
        VIGILANDO.store(false, Ordering::SeqCst);
    }
}

/// Lo que hace falta para abrir las ventanas de un grupo.
#[derive(Clone)]
pub struct Lanzador {
    pub idioma: pixpin_store::Idioma,
    pub ubicacion: Ubicacion,
    pub opciones: crate::ventana_chat::OpcionesLienzo,
}

fn lanzar(l: &Lanzador, clase: &Clase) -> bool {
    match clase {
        Clase::Lector { ruta } => {
            ruta.is_file()
                && crate::lector::abrir_en_su_lector(
                    l.idioma,
                    &l.ubicacion,
                    ruta,
                    &pixpin_docs::nombre(ruta),
                )
        }
        Clase::Lienzo {
            proyecto,
            referencia,
        } => {
            // En su hilo, como el lienzo que abre el universo: el que pide
            // el grupo (el chat o la bandeja) sigue atendiendo.
            let (raiz, proyecto, referencia, opciones) = (
                l.ubicacion.raiz().to_path_buf(),
                proyecto.clone(),
                referencia.clone(),
                l.opciones,
            );
            std::thread::Builder::new()
                .name("lienzo-del-grupo".into())
                .spawn(move || {
                    let _com = pixpin_shell::ComDelHilo::iniciar();
                    crate::abrir_hoja::abrir_hoja(&raiz, &proyecto, &referencia, opciones);
                    // La burbuja del chat tiene que ensenar lo dibujado.
                    crate::ventana_chat::refrescar();
                })
                .is_ok()
        }
        Clase::Nota { destino } => {
            crate::notas_md::abrir(l.idioma, l.ubicacion.clone(), destino.clone());
            true
        }
    }
}

/// Abre un grupo: lo que no estaba abierto se abre, y todo vuelve a su sitio.
///
/// A diferencia del movil, **no cierra** lo demas: alli la multitarea es
/// una tira de tres y abrir un grupo la sustituye; aqui cerrar ventanas del
/// usuario sin preguntar seria perderle trabajo. Devuelve cuantas se
/// abrieron o colocaron.
pub fn abrir_grupo(l: &Lanzador, g: &Grupo) -> usize {
    let mut hechas = 0;
    for v in &g.ventanas {
        let ya = esta_abierta(&v.clase);
        if ya || lanzar(l, &v.clase) {
            if let Ok(mut p) = PENDIENTES.lock() {
                p.push((v.clase.clone(), v.colocacion, Instant::now()));
            }
            hechas += 1;
        } else {
            tracing::info!(clase = ?v.clase, "una ventana del grupo ya no se puede abrir");
        }
    }
    vigilar();
    hechas
}

// ---------------------------------------------------------------------------
// El menu

const ID_GUARDAR: u32 = 1;
const ID_NADA: u32 = 2;
const ID_ABRIR: u32 = 100;
const ID_BORRAR: u32 = 300;

/// Las entradas del menu: guardar lo abierto (o decir que no hay nada),
/// abrir cada grupo y borrar cada grupo.
fn entradas(hay: usize, grupos: &[Grupo], textos: &Catalogo) -> Vec<(u32, String)> {
    let mut v: Vec<(u32, String)> = Vec::new();
    if hay == 0 {
        v.push((ID_NADA, textos.t("grupos-nada-abierto")));
    } else {
        let mut a = fluent_bundle::FluentArgs::new();
        a.set("n", hay);
        v.push((ID_GUARDAR, textos.t_args("grupos-guardar", &a)));
    }
    for (i, g) in grupos.iter().enumerate() {
        let mut a = fluent_bundle::FluentArgs::new();
        a.set("nombre", g.nombre.clone());
        a.set("n", g.ventanas.len());
        v.push((ID_ABRIR + i as u32, textos.t_args("grupos-abrir", &a)));
    }
    for (i, g) in grupos.iter().enumerate() {
        let mut a = fluent_bundle::FluentArgs::new();
        a.set("nombre", g.nombre.clone());
        v.push((ID_BORRAR + i as u32, textos.t_args("grupos-borrar", &a)));
    }
    v
}

/// El menu de los grupos, nativo y donde este el raton: guardar lo abierto,
/// abrir un grupo, borrar uno. `propietaria` es la ventana de quien lo pide
/// (el chat); sin ella (la bandeja) se usa una oculta de un momento.
///
/// Devuelve un aviso para ensenar, si hay que decir algo.
pub fn menu(propietaria: Option<isize>, l: &Lanzador, textos: &Catalogo) -> Option<String> {
    let grupos = leer(l.ubicacion.raiz());
    let hay = fotografiar();
    let lista = entradas(hay.len(), &grupos, textos);
    let elegido = match propietaria {
        Some(h) => pixpin_shell::menu_llano(windows::Win32::Foundation::HWND(h as *mut _), &lista),
        None => colocacion::menu_sin_duena(&lista),
    }?;
    match elegido {
        ID_GUARDAR => {
            pedir_nombre_y_guardar(l.ubicacion.clone(), hay, &grupos, textos);
            None
        }
        n if (ID_ABRIR..ID_BORRAR).contains(&n) => {
            let g = grupos.get((n - ID_ABRIR) as usize)?;
            let hechas = abrir_grupo(l, g);
            (hechas < g.ventanas.len()).then(|| textos.t("grupos-alguna-falta"))
        }
        n if n >= ID_BORRAR => {
            let g = grupos.get((n - ID_BORRAR) as usize)?;
            if let Err(e) = escribir(l.ubicacion.raiz(), &sin_grupo(&grupos, &g.id)) {
                tracing::warn!(?e, "no se pudo borrar el grupo");
            }
            None
        }
        _ => None,
    }
}

/// El nombre se pide en la caja de texto de Windows, en su hilo; lo abierto
/// se fotografia ANTES de preguntar, que es cuando el usuario lo pidio.
fn pedir_nombre_y_guardar(
    ubicacion: Ubicacion,
    hay: Vec<Ventana>,
    grupos: &[Grupo],
    textos: &Catalogo,
) {
    let sin_nombre = textos.t("grupos-sin-nombre");
    let caja = pixpin_shell::caja_de_texto::abrir(pixpin_shell::caja_de_texto::Pedido {
        titulo: textos.t("grupos-nombre"),
        texto: format!("{sin_nombre} {}", grupos.len() + 1),
        guardar: textos.t("grupos-guardar-boton"),
        cancelar: textos.t("chat-cancelar-caja"),
    });
    let lanzado = std::thread::Builder::new()
        .name("grupos-nombre".into())
        .spawn(move || {
            let Ok(Some(nombre)) = caja.recv() else {
                return;
            };
            let raiz = ubicacion.raiz();
            let ahora = pixpin_shell::entorno::ahora_utc_ms();
            // Se relee: mientras se escribia el nombre pudo guardarse otro.
            let nuevos = con_grupo(
                &leer(raiz),
                nombre.lines().next().unwrap_or(""),
                hay,
                ahora,
                &sin_nombre,
            );
            match escribir(raiz, &nuevos) {
                Ok(()) => tracing::info!(grupos = nuevos.len(), "grupo de ventanas guardado"),
                Err(e) => tracing::warn!(?e, "no se pudo guardar el grupo de ventanas"),
            }
        });
    if let Err(e) = lanzado {
        tracing::warn!(?e, "no se pudo esperar el nombre del grupo");
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn c(x: i32, ancho: i32) -> Colocacion {
        Colocacion {
            mostrar: 1,
            banderas: 0,
            izquierda: x,
            arriba: 0,
            derecha: x + ancho,
            abajo: 600,
            min_x: -1,
            min_y: -1,
            max_x: -1,
            max_y: -1,
        }
    }

    fn lector(ruta: &str) -> Ventana {
        Ventana {
            clase: Clase::Lector { ruta: ruta.into() },
            colocacion: c(0, 800),
        }
    }

    fn textos() -> Catalogo {
        Catalogo::nuevo(pixpin_store::Idioma::Espanol)
    }

    #[test]
    fn guardar_un_grupo_lo_pone_primero_y_sustituye_al_del_mismo_nombre() {
        let g = con_grupo(&[], "Obra", vec![lector("a.pdf")], 1, "Grupo");
        let g = con_grupo(&g, "Casa", vec![lector("b.pdf")], 2, "Grupo");
        let g = con_grupo(&g, "obra", vec![lector("c.pdf")], 3, "Grupo");
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].nombre, "obra");
        assert_eq!(g[0].ventanas, vec![lector("c.pdf")]);
        assert_eq!(g[1].nombre, "Casa");
    }

    #[test]
    fn un_nombre_en_blanco_se_llama_grupo_y_su_numero() {
        let g = con_grupo(&[], "   ", vec![], 1, "Grupo");
        assert_eq!(g[0].nombre, "Grupo 1");
    }

    #[test]
    fn no_se_guardan_mas_de_doce_y_se_va_el_mas_viejo() {
        let mut g = Vec::new();
        for i in 0..15 {
            g = con_grupo(&g, &format!("g{i}"), vec![], i, "Grupo");
        }
        assert_eq!(g.len(), GRUPOS);
        assert_eq!(g[0].nombre, "g14");
        assert!(!g.iter().any(|x| x.nombre == "g0"));
    }

    #[test]
    fn la_misma_ventana_dos_veces_se_guarda_una() {
        let g = con_grupo(
            &[],
            "x",
            vec![lector("a.pdf"), lector("a.pdf"), lector("b.pdf")],
            1,
            "Grupo",
        );
        assert_eq!(g[0].ventanas.len(), 2);
    }

    #[test]
    fn borrar_un_grupo_deja_los_demas() {
        let g = con_grupo(&[], "a", vec![], 1, "Grupo");
        let g = con_grupo(&g, "b", vec![], 2, "Grupo");
        let g = sin_grupo(&g, "g-1");
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].nombre, "b");
        assert_eq!(sin_grupo(&g, "no-existe").len(), 1);
    }

    #[test]
    fn los_grupos_se_escriben_y_se_leen_igual() {
        let dir = std::env::temp_dir().join(format!("pixpin-grupos-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let g = con_grupo(
            &[],
            "Obra",
            vec![
                lector("C:\\a.pdf"),
                Ventana {
                    clase: Clase::Lienzo {
                        proyecto: "p1".into(),
                        referencia: "d1".into(),
                    },
                    colocacion: c(10, 300),
                },
                Ventana {
                    clase: Clase::Nota {
                        destino: crate::notas_md::Destino::Mensaje {
                            proyecto: "p1".into(),
                            codigo: "U1".into(),
                        },
                    },
                    colocacion: c(20, 500),
                },
            ],
            5,
            "Grupo",
        );
        escribir(&dir, &g).unwrap();
        assert_eq!(leer(&dir), g);
        assert!(!dir.join("grupos-de-ventanas.json.tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sin_fichero_o_con_uno_roto_no_hay_grupos() {
        let dir = std::env::temp_dir().join(format!("pixpin-grupos-roto-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(leer(&dir).is_empty());
        std::fs::write(dir.join("grupos-de-ventanas.json"), "{no es json").unwrap();
        assert!(leer(&dir).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sin_nada_abierto_el_menu_no_ofrece_guardar() {
        let t = textos();
        let e = entradas(0, &[], &t);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].0, ID_NADA);
        let g = con_grupo(&[], "Obra", vec![lector("a.pdf")], 1, "Grupo");
        let e = entradas(2, &g, &t);
        assert_eq!(
            e.iter().map(|x| x.0).collect::<Vec<_>>(),
            vec![ID_GUARDAR, ID_ABRIR, ID_BORRAR]
        );
        assert!(e[1].1.contains("Obra"));
    }

    #[test]
    fn una_ventana_apuntada_deja_de_contar_al_cerrarse() {
        let clase = Clase::Lector {
            ruta: "prueba-apunte-que-no-existe.pdf".into(),
        };
        let apunte = apuntar(clase.clone());
        assert!(ABIERTAS.lock().unwrap().iter().any(|r| r.clase == clase));
        // Sin ventana todavia no cuenta como abierta: no hay nada que colocar.
        assert!(!esta_abierta(&clase));
        drop(apunte);
        assert!(!ABIERTAS.lock().unwrap().iter().any(|r| r.clase == clase));
    }

    #[test]
    fn la_colocacion_pendiente_la_toma_una_sola_ventana() {
        let clase = Clase::Lector {
            ruta: "prueba-pendiente.pdf".into(),
        };
        PENDIENTES
            .lock()
            .unwrap()
            .push((clase.clone(), c(5, 400), Instant::now()));
        assert_eq!(tomar_colocacion(&clase), Some(c(5, 400)));
        assert_eq!(tomar_colocacion(&clase), None);
    }
}
