//! **Las hojas de calculo del chat**: abrirlas, editarlas celda a celda y
//! guardarlas donde las guarda Windows (`por-traducir/pixpin/src/hoja_pin.rs`):
//!
//! - **Una tabla del PC** (mensaje `MINIAPP` con `miniapp = "tabla"`): el
//!   JSON de `TablaDeCalculo` va en el `texto` del mensaje, y guardar lo
//!   reescribe ahi con `cuaderno::cambiar` y su `tocado` claramente despues
//!   de la hora del mensaje (`editada_a`).
//! - **Una tabla del movil** (mensaje `TABLA` con `referencia`): su JSON
//!   vive en `tablas/<ref>.json` (`vista::tabla_del_movil`) y se guarda en
//!   ESE fichero (`vista::guardar_tabla_del_movil`), que es el que viaja.
//!
//! Lo que el Mac no entiende (`protegida`, claves nuevas del movil) se
//! conserva: `Tabla` y `EstiloDeCelda` guardan lo desconocido tal cual.
//!
//! # Rendimiento
//!
//! La tabla es un mapa de celda a texto (una con `A1` y `Z900` son dos
//! entradas). Swift pide **solo las celdas del rectangulo que se ve**
//! ([`HojaDeCalculo::celdas`]) y las formulas se calculan una vez por cambio
//! (`formula::evaluar_todo`), no por fotograma.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use pixpin_proyecto::formula::{self, Valor};
use pixpin_proyecto::tabla::{self, EstiloDeCelda, Ref, Tabla};
use pixpin_proyecto::{almacen, cuaderno, vista};

use crate::chats::validar_chat;
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// El formato de una celda, como lo guardan Windows y el movil.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct EstiloCeldaHoja {
    pub negrita: bool,
    pub cursiva: bool,
    pub subrayado: bool,
    pub tachado: bool,
    /// `i`, `c`, `d` o nada (la que toque por el valor).
    pub alineacion: Option<String>,
    /// El fondo, `#rrggbb`.
    pub fondo: Option<String>,
    /// El color de la letra, `#rrggbb`.
    pub color: Option<String>,
    /// Ajustar el texto (varias lineas).
    pub ajustar: bool,
    /// `numero`, `moneda`, `porcentaje`, `fecha` o nada («General»).
    pub formato_numero: Option<String>,
}

impl From<&EstiloDeCelda> for EstiloCeldaHoja {
    fn from(e: &EstiloDeCelda) -> Self {
        EstiloCeldaHoja {
            negrita: e.n,
            cursiva: e.k,
            subrayado: e.s,
            tachado: e.t,
            alineacion: e.a.clone(),
            fondo: e.f.clone(),
            color: e.c.clone(),
            ajustar: e.w,
            formato_numero: e.nf.clone(),
        }
    }
}

/// Pone en `e` lo que dice `n`, sin tocar lo que el Mac no entiende.
fn aplicar(e: &mut EstiloDeCelda, n: &EstiloCeldaHoja) {
    let limpio = |v: &Option<String>| v.clone().filter(|s| !s.trim().is_empty());
    e.n = n.negrita;
    e.k = n.cursiva;
    e.s = n.subrayado;
    e.t = n.tachado;
    e.a = limpio(&n.alineacion).filter(|a| matches!(a.as_str(), "i" | "c" | "d"));
    e.f = limpio(&n.fondo);
    e.c = limpio(&n.color);
    e.w = n.ajustar;
    e.nf = limpio(&n.formato_numero);
}

/// Una celda con algo, tal como se ensena.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CeldaHoja {
    /// Desde 0.
    pub fila: u32,
    pub columna: u32,
    /// Lo escrito (una formula empieza por `=`).
    pub contenido: String,
    /// Lo que se ensena: el resultado de la formula o el valor, con el
    /// separador decimal de este equipo (vista «General»).
    pub mostrado: String,
    /// El numero, si el valor lo es (para aplicarle su formato en Swift).
    pub numero: Option<f64>,
    /// Si el valor es un error de formula (`#¡DIV/0!`...).
    pub error: bool,
    pub estilo: Option<EstiloCeldaHoja>,
}

/// El ancho de una columna puesto a mano, en pixeles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AnchoColumna {
    pub columna: u32,
    pub ancho: u32,
}

/// De donde viene la tabla y donde se guarda.
#[derive(Debug, Clone)]
enum Origen {
    /// `MINIAPP` tabla: en el `texto` de su mensaje.
    Chat { carpeta: PathBuf, mensaje: String },
    /// `TABLA` del movil: en su fichero `tablas/<ref>.json`.
    DelMovil { archivo: PathBuf },
}

struct Estado {
    tabla: Tabla,
    /// Lo calculado, hasta el siguiente cambio.
    valores: Option<BTreeMap<String, Valor>>,
    tocada: bool,
}

/// **Una hoja de calculo del chat abierta para editar.**
#[derive(uniffi::Object)]
pub struct HojaDeCalculo {
    origen: Origen,
    estado: Mutex<Estado>,
    /// El separador decimal de este equipo para ensenar los numeros.
    decimal: char,
}

impl HojaDeCalculo {
    fn tomar(&self) -> MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn nueva(origen: Origen, tabla: Tabla) -> HojaDeCalculo {
        HojaDeCalculo {
            origen,
            estado: Mutex::new(Estado {
                tabla,
                valores: None,
                tocada: false,
            }),
            decimal: pixpin_sistema::entorno::separador_decimal(),
        }
    }
}

/// El `tocado` de una tabla guardada: ahora, pero siempre despues de la hora
/// de su mensaje (`hoja_pin::editada_a`).
fn editada_a(ahora: i64, cuando: i64) -> i64 {
    ahora.max(cuando + 2_001)
}

#[uniffi::export]
impl HojaDeCalculo {
    pub fn nombre(&self) -> String {
        self.tomar().tabla.nombre.clone()
    }

    /// Hasta donde llega lo escrito: `[columnas, filas]`.
    pub fn tamano(&self) -> Vec<u32> {
        let (c, f) = self.tomar().tabla.tamano();
        vec![c, f]
    }

    /// **Las celdas con algo del rectangulo** `filas x columnas` (desde 0,
    /// fin excluido), calculadas. Las vacias sin formato no vienen.
    pub fn celdas(
        &self,
        desde_fila: u32,
        hasta_fila: u32,
        desde_columna: u32,
        hasta_columna: u32,
    ) -> Vec<CeldaHoja> {
        let mut g = self.tomar();
        let st = &mut *g;
        if st.valores.is_none() {
            st.valores = Some(formula::evaluar_todo(&st.tabla));
        }
        let dentro = |r: &Ref| {
            (desde_fila..hasta_fila).contains(&r.fila)
                && (desde_columna..hasta_columna).contains(&r.columna)
        };
        let valores = st.valores.as_ref();
        let mut v: Vec<CeldaHoja> = Vec::new();
        let mut vistas = std::collections::BTreeSet::new();
        for (clave, contenido) in &st.tabla.celdas {
            let Some(r) = tabla::ref_de(clave) else {
                continue;
            };
            if !dentro(&r) {
                continue;
            }
            let valor = valores
                .and_then(|m| m.get(clave).cloned())
                .unwrap_or_else(|| formula::literal(contenido));
            vistas.insert(clave.clone());
            v.push(CeldaHoja {
                fila: r.fila,
                columna: r.columna,
                contenido: contenido.clone(),
                mostrado: valor.mostrar(self.decimal),
                numero: match valor {
                    Valor::Numero(n) => Some(n),
                    _ => None,
                },
                error: matches!(valor, Valor::Error(_)),
                estilo: st.tabla.estilos.get(clave).map(EstiloCeldaHoja::from),
            });
        }
        // Las vacias con formato (un fondo puesto) tambien se ven.
        for (clave, e) in &st.tabla.estilos {
            if vistas.contains(clave) || e.sin_formato() {
                continue;
            }
            let Some(r) = tabla::ref_de(clave) else {
                continue;
            };
            if dentro(&r) {
                v.push(CeldaHoja {
                    fila: r.fila,
                    columna: r.columna,
                    contenido: String::new(),
                    mostrado: String::new(),
                    numero: None,
                    error: false,
                    estilo: Some(EstiloCeldaHoja::from(e)),
                });
            }
        }
        v
    }

    /// Lo escrito en una celda.
    pub fn contenido(&self, fila: u32, columna: u32) -> String {
        self.tomar().tabla.celda(Ref { columna, fila }).to_string()
    }

    /// **Escribe una celda** (vacio la quita). Devuelve si cambio algo.
    pub fn poner(&self, fila: u32, columna: u32, contenido: String) -> bool {
        let mut st = self.tomar();
        let r = Ref { columna, fila };
        if st.tabla.celda(r) == contenido {
            return false;
        }
        st.tabla.poner(r, &contenido);
        st.valores = None;
        st.tocada = true;
        true
    }

    /// El formato de una celda (`None` sin formato).
    pub fn estilo(&self, fila: u32, columna: u32) -> Option<EstiloCeldaHoja> {
        let clave = tabla::ref_a(Ref { columna, fila });
        self.tomar()
            .tabla
            .estilos
            .get(&clave)
            .map(EstiloCeldaHoja::from)
    }

    /// **Pone el formato de una celda.** Uno sin nada que se vea (y sin lo
    /// que el Mac no entiende) se quita en vez de guardarse vacio.
    pub fn poner_estilo(&self, fila: u32, columna: u32, estilo: EstiloCeldaHoja) -> bool {
        let clave = tabla::ref_a(Ref { columna, fila });
        let mut st = self.tomar();
        let mut e = st.tabla.estilos.get(&clave).cloned().unwrap_or_default();
        let antes = e.clone();
        aplicar(&mut e, &estilo);
        if e == antes {
            return false;
        }
        if e.vacio() {
            st.tabla.estilos.remove(&clave);
        } else {
            st.tabla.estilos.insert(clave, e);
        }
        st.tocada = true;
        true
    }

    /// Los anchos puestos a mano.
    pub fn anchos(&self) -> Vec<AnchoColumna> {
        self.tomar()
            .tabla
            .anchos
            .iter()
            .filter_map(|(k, v)| {
                let r = tabla::ref_de(&format!("{k}1"))?;
                Some(AnchoColumna {
                    columna: r.columna,
                    ancho: *v,
                })
            })
            .collect()
    }

    /// Pone el ancho de una columna (`None` vuelve al de siempre).
    pub fn poner_ancho(&self, columna: u32, ancho: Option<u32>) -> bool {
        let letras = tabla::ref_a(Ref { columna, fila: 0 })
            .trim_end_matches('1')
            .to_string();
        let mut st = self.tomar();
        let antes = st.tabla.anchos.get(&letras).copied();
        let nuevo = ancho.map(|a| a.clamp(16, 2000));
        if antes == nuevo {
            return false;
        }
        match nuevo {
            Some(a) => st.tabla.anchos.insert(letras, a),
            None => st.tabla.anchos.remove(&letras),
        };
        st.tocada = true;
        true
    }

    /// Si hay cambios sin guardar.
    pub fn tocada(&self) -> bool {
        self.tomar().tocada
    }

    /// **Guarda** donde vive la tabla (ver la doc del modulo). Devuelve si
    /// escribio algo. Bloqueante: Swift lo llama fuera del hilo principal.
    pub fn guardar(&self) -> Result<bool, ErrorNucleo> {
        let mut st = self.tomar();
        if !st.tocada {
            return Ok(false);
        }
        let ahora = crate::ahora_ms();
        match &self.origen {
            Origen::DelMovil { archivo } => {
                st.tabla.tocado = st.tabla.tocado.max(ahora);
                let texto = st.tabla.escribir().map_err(|e| ErrorNucleo::Disco {
                    detalle: e.to_string(),
                })?;
                vista::guardar_tabla_del_movil(archivo, &texto)?;
            }
            Origen::Chat { carpeta, mensaje } => {
                let tabla = &mut st.tabla;
                let mut fallo = None;
                let hecho = cuaderno::cambiar(carpeta, mensaje, |m| {
                    tabla.tocado = editada_a(ahora, m.cuando);
                    match tabla.escribir() {
                        Ok(t) => {
                            m.texto = t;
                            true
                        }
                        Err(e) => {
                            fallo = Some(e.to_string());
                            false
                        }
                    }
                })?;
                if let Some(detalle) = fallo {
                    return Err(ErrorNucleo::Disco { detalle });
                }
                // Se borro del chat mientras se editaba: no se resucita.
                if hecho.is_none() {
                    return Err(ErrorNucleo::NoExiste {
                        que: mensaje.clone(),
                    });
                }
            }
        }
        st.tocada = false;
        Ok(true)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Abre la hoja de calculo de un mensaje**: una tabla del PC (`MINIAPP`
    /// tabla) o del movil (`TABLA`). `NoExiste` si el mensaje no esta o su
    /// fichero aun no llego (mejor no abrir que abrir en blanco y guardar
    /// encima); `Argumento` si no es una tabla.
    pub fn abrir_hoja_de_calculo(
        &self,
        chat: String,
        mensaje: String,
    ) -> Result<Arc<HojaDeCalculo>, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let carpeta = almacen::carpeta(raiz, &chat);
        let c = cuaderno::Cuaderno::leer_de(&carpeta)?;
        let m = c
            .mensajes
            .into_iter()
            .find(|m| m.id == mensaje)
            .ok_or_else(|| ErrorNucleo::NoExiste {
                que: mensaje.clone(),
            })?;
        let no_vale = |d: String| ErrorNucleo::Argumento { detalle: d };
        if let Some(archivo) = vista::tabla_del_movil(raiz, &chat, &m) {
            let texto = std::fs::read_to_string(&archivo).map_err(|_| ErrorNucleo::NoExiste {
                que: archivo.display().to_string(),
            })?;
            let t = Tabla::leer(&texto).map_err(|e| no_vale(e.to_string()))?;
            return Ok(Arc::new(HojaDeCalculo::nueva(
                Origen::DelMovil { archivo },
                t,
            )));
        }
        if m.clase == Some(cuaderno::Clase::MiniApp) && m.miniapp.as_deref() == Some(tabla::MINIAPP)
        {
            let t = Tabla::leer(&m.texto).map_err(|e| no_vale(e.to_string()))?;
            return Ok(Arc::new(HojaDeCalculo::nueva(
                Origen::Chat {
                    carpeta,
                    mensaje: m.id,
                },
                t,
            )));
        }
        Err(no_vale(format!("no es una tabla: {mensaje}")))
    }

    /// Si el mensaje es una hoja de calculo que se abre con
    /// [`Nucleo::abrir_hoja_de_calculo`] (sin leer su tabla).
    pub fn es_hoja_de_calculo(&self, chat: String, mensaje: String) -> bool {
        if validar_chat(&chat).is_err() {
            return false;
        }
        let carpeta = almacen::carpeta(self.ruta(), &chat);
        cuaderno::Cuaderno::leer_de(&carpeta).is_ok_and(|c| {
            c.mensajes.iter().any(|m| {
                m.id == mensaje
                    && ((m.clase == Some(cuaderno::Clase::MiniApp)
                        && m.miniapp.as_deref() == Some(tabla::MINIAPP))
                        || m.clase == Some(cuaderno::Clase::Otra(vista::CLASE_TABLA.into())))
            })
        })
    }

    /// **Una tabla nueva en un chat**: un mensaje `MINIAPP` tabla con la
    /// tabla vacia, como la crea Windows. Se abre despues con
    /// [`Nucleo::abrir_hoja_de_calculo`].
    pub fn crear_tabla(&self, chat: String, nombre: String) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let indice = almacen::Indice::leer_si_esta(raiz)?;
        if indice.buscar(&chat).is_none() {
            return Err(ErrorNucleo::NoExiste { que: chat });
        }
        let nombre = {
            let n = nombre.trim();
            if n.is_empty() {
                "Tabla".to_string()
            } else {
                n.chars().take(80).collect()
            }
        };
        let cuando = crate::chats::hora_unica();
        let t = Tabla {
            nombre: nombre.clone(),
            tocado: cuando,
            ..Default::default()
        };
        let documento = t.escribir().map_err(|e| ErrorNucleo::Disco {
            detalle: e.to_string(),
        })?;
        let carpeta = almacen::carpeta(raiz, &chat);
        let fichero = carpeta.join("guardados.jsonl");
        let aparato = self.identidad()?.yo.codigo();
        let mut cache = self.cache_tomada();
        let numero = cache.indice(&fichero)?.map_or(0, |i| i.mayor_numero) + 1;
        let m = cuaderno::Mensaje::miniapp(
            tabla::MINIAPP,
            &nombre,
            &documento,
            &cuaderno::Sello {
                cuando,
                numero,
                aparato,
                proyecto: chat.clone(),
            },
        );
        cuaderno::anadir(&carpeta, &m)?;
        cache.quitar(&fichero);
        drop(cache);
        let mut indice = indice;
        if let Some(f) = indice.proyectos.iter_mut().find(|f| f.id == chat) {
            f.tocado = cuando;
            f.resumen = m.resumen();
        }
        indice.guardar(raiz)?;
        Ok(MensajeChat::from(m))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn nucleo(n: &str) -> (PathBuf, Arc<Nucleo>) {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-hojas-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let nu = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        (d, nu)
    }

    #[test]
    fn una_tabla_del_pc_se_edita_y_vuelve_al_texto_de_su_mensaje() {
        let (d, n) = nucleo("pc");
        let chat = n.crear_chat("Cuentas".into()).unwrap();
        let m = n.crear_tabla(chat.id.clone(), "Gastos".into()).unwrap();
        assert_eq!(m.clase, "MINIAPP");
        assert!(n.es_hoja_de_calculo(chat.id.clone(), m.id.clone()));
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h.nombre(), "Gastos");
        assert!(!h.guardar().unwrap(), "sin cambios no escribe");
        assert!(h.poner(0, 0, "12.5".into()));
        assert!(h.poner(1, 0, "7".into()));
        assert!(h.poner(2, 0, "=SUMA(A1:A2)".into()));
        assert!(h.poner_estilo(
            2,
            0,
            EstiloCeldaHoja {
                negrita: true,
                formato_numero: Some("moneda".into()),
                ..Default::default()
            }
        ));
        let total = h
            .celdas(0, 10, 0, 5)
            .into_iter()
            .find(|c| c.fila == 2)
            .unwrap();
        assert_eq!(total.numero, Some(19.5));
        assert!(total.estilo.as_ref().unwrap().negrita);
        assert!(h.guardar().unwrap());
        // En el cuaderno: el JSON de `TablaDeCalculo` en el texto, con los
        // nombres cortos del movil y del PC.
        let carpeta = almacen::carpeta(&d, &chat.id);
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let guardado = c.mensajes.iter().find(|x| x.id == m.id).unwrap();
        let t = Tabla::leer(&guardado.texto).unwrap();
        assert_eq!(t.celdas.get("A3").map(String::as_str), Some("=SUMA(A1:A2)"));
        assert!(t.estilos["A3"].n);
        assert_eq!(t.estilos["A3"].nf.as_deref(), Some("moneda"));
        assert!(t.tocado >= guardado.cuando + 2_001);
        // Reabrir lee lo guardado; vaciar una celda la quita.
        let h2 = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert_eq!(h2.contenido(0, 0), "12.5");
        assert!(h2.poner(0, 0, String::new()));
        assert!(h2.guardar().unwrap());
        let c = cuaderno::Cuaderno::leer_de(&carpeta).unwrap();
        let t = Tabla::leer(&c.mensajes.iter().find(|x| x.id == m.id).unwrap().texto).unwrap();
        assert!(!t.celdas.contains_key("A1"));
        // Caso negativo: una nota no es una tabla.
        let nota = n
            .escribir_nota(chat.id.clone(), "hola".into(), None)
            .unwrap();
        assert!(!n.es_hoja_de_calculo(chat.id.clone(), nota.id.clone()));
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.id, nota.id),
            Err(ErrorNucleo::Argumento { .. })
        ));
    }

    #[test]
    fn una_tabla_del_movil_se_guarda_en_su_fichero_y_conserva_lo_desconocido() {
        let (d, n) = nucleo("movil");
        let chat = n.crear_chat("Obra".into()).unwrap();
        // El mensaje `TABLA` del movil y su fichero `tablas/<ref>.json`.
        let carpeta = almacen::carpeta(&d, &chat.id);
        let m = cuaderno::Mensaje {
            id: "1700000000000".into(),
            cuando: 1_700_000_000_000,
            clase: Some(cuaderno::Clase::Otra(vista::CLASE_TABLA.into())),
            referencia: Some("t-medidas".into()),
            nombre: "Medidas".into(),
            numero: 1,
            uid: Some("ABCDEFGHJK".into()),
            proyecto: Some(chat.id.clone()),
            ..Default::default()
        };
        cuaderno::anadir(&carpeta, &m).unwrap();
        let archivo = vista::tabla_del_movil(&d, &chat.id, &m).unwrap();
        // Aun no llego el fichero: no se abre en blanco.
        assert!(matches!(
            n.abrir_hoja_de_calculo(chat.id.clone(), m.id.clone()),
            Err(ErrorNucleo::NoExiste { .. })
        ));
        std::fs::create_dir_all(archivo.parent().unwrap()).unwrap();
        std::fs::write(
            &archivo,
            r#"{"nombre":"Medidas","celdas":{"B2":"3"},"anchos":{},"estilos":{"B2":{"n":true}},"tocado":5,"protegida":true}"#,
        )
        .unwrap();
        let h = n
            .abrir_hoja_de_calculo(chat.id.clone(), m.id.clone())
            .unwrap();
        assert!(h.poner(1, 1, "4".into()));
        assert!(h.poner_ancho(1, Some(120)));
        assert!(h.guardar().unwrap());
        let t = Tabla::leer(&std::fs::read_to_string(&archivo).unwrap()).unwrap();
        assert_eq!(t.celdas["B2"], "4");
        assert!(t.estilos["B2"].n);
        assert_eq!(t.anchos["B"], 120);
        assert_eq!(
            t.resto.get("protegida"),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(t.tocado > 5);
        assert_eq!(
            h.anchos(),
            vec![AnchoColumna {
                columna: 1,
                ancho: 120
            }]
        );
    }
}
