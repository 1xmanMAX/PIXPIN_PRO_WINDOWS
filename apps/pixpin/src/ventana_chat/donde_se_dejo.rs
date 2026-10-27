//! **Cada chat se abre donde se dejo** (27-oct-2026, checklist del usuario,
//! punto 4; Android 00ac81c2, `PosicionDelChat.kt`). Al salir de un chat
//! subido se apunta el mensaje que asomaba arriba y cuanto de el quedaba por
//! encima; al volver se va ahi. Dejado abajo, nuevo, o si aquel mensaje ya no
//! esta, se abre pegado al final como siempre. Y lo que llego por debajo del
//! ultimo mensaje visto es la chapa del «↓» (`al_final`).
//!
//! Vive en `<raiz>/chats-donde-se-dejo.json`, **solo en este equipo**: no se
//! sincroniza (donde se quedo cada chat es de cada aparato). Un fichero roto o
//! que no esta es «todos abajo», nunca un error. Las cuentas son de
//! `pixpin_ui::donde_se_dejo`; aqui solo el disco y el apunte por chat.
//!
//! Lo de cada fotograma va a memoria (un `Mutex` por raiz) y solo se escribe
//! al cerrar el chat (`Drop` de [`Recuerdo`]): abrir otro chat lee de esa
//! memoria, asi que da igual en que orden se suelte el anterior.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pixpin_geom::Rect;
use pixpin_ui::donde_se_dejo::{self as cuentas, AlAbrir, Guardada};
use pixpin_ui::historial::Puesto;
use serde::{Deserialize, Serialize};

pub(super) const NOMBRE: &str = "chats-donde-se-dejo.json";

/// Como va cada chat en el fichero.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EnDisco {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arriba: Option<String>,
    #[serde(default)]
    desplazamiento: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    leido_hasta: Option<String>,
}

/// Lo de una raiz: los chats y si hay algo sin escribir.
#[derive(Default)]
struct Libro {
    chats: BTreeMap<String, Guardada>,
    sucio: bool,
}

/// Una raiz por libro, leida la primera vez que se pide.
static MEMORIA: Mutex<BTreeMap<PathBuf, Libro>> = Mutex::new(BTreeMap::new());

fn ruta(raiz: &Path) -> PathBuf {
    raiz.join(NOMBRE)
}

fn leer_del_disco(raiz: &Path) -> BTreeMap<String, Guardada> {
    std::fs::read_to_string(ruta(raiz))
        .ok()
        .and_then(|t| serde_json::from_str::<BTreeMap<String, EnDisco>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|(chat, d)| {
            (
                chat,
                Guardada {
                    arriba: d.arriba,
                    desplazamiento: d.desplazamiento,
                    leido_hasta: d.leido_hasta,
                },
            )
        })
        .collect()
}

/// Con el libro de `raiz` en la mano (leido del disco si aun no estaba).
fn con_libro<T>(raiz: &Path, f: impl FnOnce(&mut Libro) -> T) -> T {
    let mut m = MEMORIA.lock().unwrap_or_else(|e| e.into_inner());
    let libro = m.entry(raiz.to_path_buf()).or_insert_with(|| Libro {
        chats: leer_del_disco(raiz),
        sucio: false,
    });
    f(libro)
}

/// Lo apuntado de un chat.
fn leer(raiz: &Path, chat: &str) -> Option<Guardada> {
    con_libro(raiz, |l| l.chats.get(chat).cloned())
}

/// Apunta en memoria; se escribe con [`guardar`].
fn apuntar_en(raiz: &Path, chat: &str, g: Guardada) {
    con_libro(raiz, |l| {
        if l.chats.get(chat) != Some(&g) {
            l.chats.insert(chat.to_owned(), g);
            l.sucio = true;
        }
    });
}

/// Escribe el fichero si cambio algo (de un tiron, por un `.tmp`).
fn guardar(raiz: &Path) {
    let texto = con_libro(raiz, |l| {
        if !l.sucio {
            return None;
        }
        l.sucio = false;
        let en_disco: BTreeMap<&String, EnDisco> = l
            .chats
            .iter()
            .map(|(chat, g)| {
                (
                    chat,
                    EnDisco {
                        arriba: g.arriba.clone(),
                        desplazamiento: g.desplazamiento,
                        leido_hasta: g.leido_hasta.clone(),
                    },
                )
            })
            .collect();
        serde_json::to_string_pretty(&en_disco).ok()
    });
    let Some(t) = texto else {
        return;
    };
    let destino = ruta(raiz);
    let tmp = destino.with_extension("json.tmp");
    if std::fs::write(&tmp, t).is_err() || std::fs::rename(&tmp, &destino).is_err() {
        let _ = std::fs::remove_file(&tmp);
        tracing::debug!(?destino, "no se pudo guardar donde se dejo cada chat");
    }
}

/// **El recuerdo del chat abierto** (va en `Abierto`): si falta volver a
/// donde se dejo y cuanto hay sin ver para la chapa del «↓». Al soltarse
/// (cerrar o cambiar de chat, cerrar la ventana) escribe lo apuntado.
pub(super) struct Recuerdo {
    raiz: PathBuf,
    chat: String,
    /// Aun no se ha vuelto a donde se dejo: se hace en cuanto las burbujas
    /// esten medidas (como `Abierto::ir_a`).
    por_volver: Cell<bool>,
    /// Los mensajes que hay por debajo del ultimo visto.
    sin_ver: Cell<usize>,
}

impl Recuerdo {
    pub(super) fn nuevo(raiz: &Path, chat: &str) -> Recuerdo {
        Recuerdo {
            raiz: raiz.to_path_buf(),
            chat: chat.to_owned(),
            por_volver: Cell::new(true),
            sin_ver: Cell::new(0),
        }
    }

    pub(super) fn por_volver(&self) -> bool {
        self.por_volver.get()
    }

    /// Un salto pedido (una cita, la busqueda, el fijado) manda sobre la
    /// vuelta a donde se dejo.
    pub(super) fn olvidar_la_vuelta(&self) {
        self.por_volver.set(false);
    }

    pub(super) fn sin_ver(&self) -> usize {
        self.sin_ver.get()
    }

    /// **La vuelta**, ya medidas las burbujas: el desplazamiento del chat
    /// (`None`, pegado al final). `ids` van en paralelo a `puestos`.
    pub(super) fn volver(
        &self,
        area: Rect,
        puestos: &[Puesto],
        alto_total: u32,
        ids: &[&str],
        escala: u32,
    ) -> Option<i32> {
        self.por_volver.set(false);
        let g = leer(&self.raiz, &self.chat);
        self.sin_ver.set(cuentas::sin_leer(
            ids,
            g.as_ref().and_then(|g| g.leido_hasta.as_deref()),
        ));
        match cuentas::al_abrir(g.as_ref(), ids) {
            AlAbrir::Abajo => None,
            AlAbrir::En {
                indice,
                desplazamiento,
            } => cuentas::scroll_del_ancla(
                area,
                puestos,
                alto_total,
                indice,
                cuentas::a_pixeles(desplazamiento, escala),
            ),
        }
    }

    /// **El apunte de cada fotograma**: el ancla de lo que se ve, el ultimo
    /// visto (que nunca vuelve atras) y, con ello, lo que queda sin ver.
    pub(super) fn apuntar(
        &self,
        area: Rect,
        puestos: &[Puesto],
        alto_total: u32,
        scroll: Option<i32>,
        ids: &[&str],
        escala: u32,
    ) {
        if self.por_volver.get() || ids.len() != puestos.len() {
            return;
        }
        let antes = leer(&self.raiz, &self.chat).unwrap_or_default();
        let visto = cuentas::ultimo_visto(area, puestos, alto_total, scroll).map(|i| ids[i]);
        let leido = cuentas::mas_avanzado(ids, antes.leido_hasta.as_deref(), visto);
        self.sin_ver.set(cuentas::sin_leer(ids, leido));
        let (arriba, desplazamiento) = match cuentas::ancla(area, puestos, alto_total, scroll) {
            Some((i, d)) => (Some(ids[i].to_owned()), cuentas::a_logicos(d, escala)),
            None => (None, 0),
        };
        let leido_hasta = leido.map(str::to_owned);
        apuntar_en(
            &self.raiz,
            &self.chat,
            Guardada {
                arriba,
                desplazamiento,
                leido_hasta,
            },
        );
    }
}

impl Drop for Recuerdo {
    fn drop(&mut self) {
        guardar(&self.raiz);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn puestos(n: i32) -> Vec<Puesto> {
        (0..n)
            .map(|i| Puesto {
                burbuja: Rect {
                    x: 0,
                    y: i * 100,
                    ancho: 200,
                    alto: 90,
                },
                separador: None,
                nombre: None,
            })
            .collect()
    }

    #[test]
    fn se_guarda_al_cerrar_y_se_vuelve_al_abrir_con_lo_no_visto() {
        let raiz = std::env::temp_dir().join(format!("pixpin-donde-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&raiz);
        std::fs::create_dir_all(&raiz).unwrap();
        let area = Rect {
            x: 0,
            y: 0,
            ancho: 700,
            alto: 600,
        };
        let ids: Vec<String> = (0..10).map(|i| format!("m{i}")).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let p = puestos(10);
        {
            let r = Recuerdo::nuevo(&raiz, "chat");
            // Nuevo: abajo.
            assert_eq!(r.volver(area, &p, 1000, &ids, 100), None);
            r.apuntar(area, &p, 1000, None, &ids, 100);
            assert_eq!(r.sin_ver(), 0);
            // Sube a 250 (asoma el m2 con 50 por encima).
            r.apuntar(area, &p, 1000, Some(250), &ids, 100);
        }
        assert!(ruta(&raiz).exists(), "se escribe al soltarse");
        // Llegan dos mensajes mientras estaba cerrado.
        let ids2: Vec<String> = (0..12).map(|i| format!("m{i}")).collect();
        let ids2: Vec<&str> = ids2.iter().map(String::as_str).collect();
        let p2 = puestos(12);
        let r = Recuerdo::nuevo(&raiz, "chat");
        assert!(r.por_volver());
        assert_eq!(r.volver(area, &p2, 1200, &ids2, 100), Some(250));
        assert_eq!(r.sin_ver(), 2, "visto hasta el m9");
        // Otro chat no se entera.
        assert_eq!(
            Recuerdo::nuevo(&raiz, "otro").volver(area, &p2, 1200, &ids2, 100),
            None
        );
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
