//! **El texto de un pin, leido en otro hilo** (el modo texto y «Copiar
//! texto»).
//!
//! Antes se reconocia en el bucle principal: con el OCR de Windows eran
//! 170-670 ms de pines, bandeja y atajos congelados; con Indra en un Core i3
//! de tercera generacion, 0,3-1,5 s por imagen y otro 0,5-1 s la primera vez
//! (cargar los modelos). Ahora:
//!
//! 1. El pin pide y sigue respondiendo; en modo texto su barra dice
//!    «Leyendo…» hasta que llega.
//! 2. Un hilo lee la imagen del ALMACEN (la original, no lo que se ve) y la
//!    reconoce; al acabar deja el resultado en un buzon y despierta al bucle,
//!    como las fotos de `pines::foto`.
//! 3. El bucle se lo da al pin **solo si sigue siendo para el**: cada pedido
//!    lleva su boleto, y lo que llegue de un pedido que ya no esta (el pin se
//!    cerro, o se pidio otra vez para otra pagina) se tira. El pin, ademas,
//!    descarta lo que traiga una vuelta vieja (`Pin::poner_texto_reconocido`).
//!
//! Si se pide dos veces mientras se lee (pasar a modo texto y, a la vez,
//! «Copiar texto»), no se lee dos veces: el segundo pedido se suma al primero.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use anyhow::Result;
use pixpin_store::TipoEntrada;

use super::Pines;

/// Un pedido en marcha para un pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pedido {
    /// Unico en toda la sesion: distingue este pedido de uno anterior para
    /// el mismo pin (que puede ser el mismo id si se cerro y se reabrio).
    pub boleto: u64,
    /// La vuelta del pin al pedirlo (`Pin::vuelta_ocr`).
    pub vuelta: u64,
    /// Ademas de darselo al pin, copiar el texto al acabar.
    pub copiar: bool,
}

/// Los pedidos en marcha, uno por pin como mucho. Sin Windows: se prueba.
#[derive(Debug, Default)]
pub struct EnCurso {
    por_pin: HashMap<u64, Pedido>,
    ultimo_boleto: u64,
}

impl EnCurso {
    /// Apunta un pedido para el pin `id`. Devuelve el boleto si hay que
    /// lanzar una lectura; `None` si ya se esta leyendo lo mismo (entonces
    /// solo se suma el `copiar`).
    pub fn pedir(&mut self, id: u64, vuelta: u64, copiar: bool) -> Option<u64> {
        if let Some(p) = self.por_pin.get_mut(&id)
            && p.vuelta == vuelta
        {
            p.copiar |= copiar;
            return None;
        }
        self.ultimo_boleto += 1;
        let boleto = self.ultimo_boleto;
        self.por_pin.insert(
            id,
            Pedido {
                boleto,
                vuelta,
                copiar,
            },
        );
        Some(boleto)
    }

    /// Llega lo leido para `id` con `boleto`: el pedido, si sigue vigente
    /// (y deja de estar en marcha); `None` si es de uno que ya no vale.
    pub fn llegar(&mut self, id: u64, boleto: u64) -> Option<Pedido> {
        match self.por_pin.get(&id) {
            Some(p) if p.boleto == boleto => self.por_pin.remove(&id),
            _ => None,
        }
    }

    /// El pin se cerro: lo que llegue para el ya no es de nadie.
    pub fn olvidar(&mut self, id: u64) {
        self.por_pin.remove(&id);
    }

    #[cfg(test)]
    fn en_marcha(&self) -> usize {
        self.por_pin.len()
    }
}

/// Lo que el hilo devuelve.
pub struct Leido {
    id: u64,
    boleto: u64,
    lineas: Vec<pixpin_ocr::Linea>,
    ms: u64,
}

/// El buzon de los textos que se leen en otro hilo, con lo que hay en marcha.
pub struct Lecturas {
    en_curso: EnCurso,
    enviar: Sender<Leido>,
    recibir: Receiver<Leido>,
}

impl Lecturas {
    pub fn nuevas() -> Lecturas {
        let (enviar, recibir) = channel();
        Lecturas {
            en_curso: EnCurso::default(),
            enviar,
            recibir,
        }
    }
}

/// Lee y reconoce la imagen del almacen. En el hilo: no toca nada del gestor.
fn reconocer_fichero(objeto: &std::path::Path, id: u64) -> Vec<pixpin_ocr::Linea> {
    // Un fallo aqui no puede tumbar nada: el pin sigue siendo un pin
    // aunque no se le pueda leer el texto. Se anota y se da una lista
    // vacia para no volver a intentarlo en cada movimiento.
    match pixpin_codec::cargar(objeto) {
        Err(e) => {
            tracing::warn!(?e, id, "no se pudo leer la imagen para reconocer");
            Vec::new()
        }
        Ok(img) => match pixpin_ocr::reconocer(img.ancho, img.alto, &img.pixeles) {
            Err(e) => {
                tracing::warn!(?e, id, "no se pudo reconocer el texto del pin");
                Vec::new()
            }
            Ok(lineas) => lineas,
        },
    }
}

impl Pines {
    /// Pide leer el texto de la imagen del pin `id` en otro hilo; con
    /// `copiar`, ademas se copia al acabar (P4.2). Lo leido se le da al pin
    /// para que se pueda seleccionar con el raton (P4.4).
    ///
    /// Se lee del ALMACEN y no de lo que se ve en pantalla: el pin puede
    /// estar reducido, girado o con algo dibujado encima, y reconocer eso
    /// daria peor texto que reconocer el original.
    pub(super) fn leer_texto_aparte(&mut self, id: u64, copiar: bool) -> Result<()> {
        let Some(pin) = self.vivos.get(&id) else {
            return Ok(());
        };
        let objeto: PathBuf = {
            let a = self.almacen.borrow();
            let Some(e) = a.entradas().iter().find(|e| e.id == id) else {
                return Ok(());
            };
            if e.tipo != TipoEntrada::Imagen {
                return Ok(());
            }
            a.ruta_objeto(e)
        };
        let vuelta = pin.vuelta_ocr();
        let Some(boleto) = self.textos_leidos.en_curso.pedir(id, vuelta, copiar) else {
            tracing::debug!(id, copiar, "el texto del pin ya se esta leyendo");
            return Ok(());
        };
        let enviar = self.textos_leidos.enviar.clone();
        // Un HWND no cruza hilos; su valor si, y PostMessage vale desde
        // cualquiera.
        let aviso = self.hwnd_app.0 as isize;
        let lanzado = std::thread::Builder::new()
            .name("pin-texto".into())
            .spawn(move || {
                // El OCR de Windows es WinRT: el hilo necesita su COM.
                let _com = pixpin_shell::ComDelHilo::iniciar();
                let t = Instant::now();
                let lineas = reconocer_fichero(&objeto, id);
                let ms = t.elapsed().as_millis() as u64;
                let leido = Leido {
                    id,
                    boleto,
                    lineas,
                    ms,
                };
                if enviar.send(leido).is_ok() {
                    pixpin_shell::despertar(windows::Win32::Foundation::HWND(aviso as *mut _));
                }
            });
        if let Err(e) = lanzado {
            // Sin hilo no hay lectura: se le da al pin un «no hay texto»
            // para que no se quede diciendo «Leyendo…» para siempre.
            tracing::warn!(?e, id, "no se pudo lanzar la lectura del texto");
            self.textos_leidos.en_curso.olvidar(id);
            pin.poner_texto_reconocido(vuelta, Vec::new());
        }
        Ok(())
    }

    /// Los textos que acabaron de leerse: cada uno a su pin, si sigue siendo
    /// para el.
    pub(super) fn atender_textos_leidos(&mut self) {
        while let Ok(l) = self.textos_leidos.recibir.try_recv() {
            self.atender_texto_leido(l);
        }
    }

    /// Un pin cerrado: lo que se este leyendo para el se tirara al llegar.
    pub(super) fn olvidar_texto_de(&mut self, id: u64) {
        self.textos_leidos.en_curso.olvidar(id);
    }

    fn atender_texto_leido(
        &mut self,
        Leido {
            id,
            boleto,
            lineas,
            ms,
        }: Leido,
    ) {
        let Some(pedido) = self.textos_leidos.en_curso.llegar(id, boleto) else {
            tracing::info!(id, boleto, "texto de un pedido que ya no vale; se tira");
            return;
        };
        let Some(pin) = self.vivos.get(&id) else {
            return;
        };
        let renglones: Vec<_> = lineas
            .iter()
            .map(|l| pixpin_geom::seleccion_texto::Renglon {
                palabras: l
                    .palabras
                    .iter()
                    .map(|p| pixpin_geom::seleccion_texto::Palabra {
                        caja: p.caja,
                        texto: p.texto.clone(),
                    })
                    .collect(),
            })
            .filter(|r| !r.palabras.is_empty())
            .collect();
        // Se guarda SIEMPRE, aunque no haya salido texto: una lista vacia
        // significa «se miro y no hay», y sin distinguirlo de «no se ha
        // mirado» una imagen sin letras se reconoceria otra vez.
        if !pin.poner_texto_reconocido(pedido.vuelta, renglones) {
            tracing::info!(id, "el pin ya ensena otra cosa; su texto se tira");
            return;
        }
        tracing::info!(id, renglones = lineas.len(), ms, "texto del pin reconocido");
        if pedido.copiar {
            // Reconocido UNA vez y usado dos: se copia y ademas se le deja
            // al pin para marcarlo con el raton sin volver a esperar.
            let texto = crate::texto_de_lineas(lineas);
            if texto.trim().is_empty() {
                tracing::info!(id, "no se leyo texto en el pin");
                return;
            }
            match pixpin_codec::copiar_texto(&texto) {
                Ok(()) => tracing::info!(id, largo = texto.len(), "texto del pin copiado"),
                Err(e) => tracing::warn!(?e, id, "no se pudo copiar el texto"),
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_pedido_llega_una_vez_y_con_su_copiar() {
        let mut c = EnCurso::default();
        let b = c.pedir(7, 0, false).expect("el primero se lanza");
        // Copiar texto mientras se lee: no se lee otra vez, se suma.
        assert_eq!(c.pedir(7, 0, true), None);
        let p = c.llegar(7, b).expect("vigente");
        assert!(p.copiar, "el segundo pedido se sumo");
        assert_eq!(p.vuelta, 0);
        // Caso negativo: lo mismo otra vez ya no es de nadie.
        assert_eq!(c.llegar(7, b), None);
        assert_eq!(c.en_marcha(), 0);
    }

    #[test]
    fn caso_negativo_lo_de_un_pin_cerrado_se_tira() {
        let mut c = EnCurso::default();
        let b = c.pedir(3, 0, true).unwrap();
        c.olvidar(3);
        assert_eq!(c.llegar(3, b), None);
        // Reabierto con el mismo id y pedido otra vez: lo viejo sigue sin
        // valer, lo nuevo si.
        let nuevo = c.pedir(3, 0, false).unwrap();
        assert_ne!(nuevo, b);
        assert_eq!(c.llegar(3, b), None);
        assert!(c.llegar(3, nuevo).is_some());
    }

    #[test]
    fn caso_negativo_otra_vuelta_relanza_y_lo_viejo_no_vale() {
        let mut c = EnCurso::default();
        let viejo = c.pedir(5, 0, false).unwrap();
        // Lo que se ve cambio (otra pagina): hay que leer lo nuevo.
        let nuevo = c.pedir(5, 1, false).expect("otra imagen, otra lectura");
        assert_eq!(c.llegar(5, viejo), None);
        assert_eq!(c.llegar(5, nuevo).map(|p| p.vuelta), Some(1));
    }

    #[test]
    fn cada_pin_lleva_lo_suyo() {
        let mut c = EnCurso::default();
        let a = c.pedir(1, 0, false).unwrap();
        let b = c.pedir(2, 0, true).unwrap();
        assert_eq!(c.llegar(1, b), None, "el boleto de otro pin no vale");
        assert!(c.llegar(2, b).unwrap().copiar);
        assert!(!c.llegar(1, a).unwrap().copiar);
    }
}
