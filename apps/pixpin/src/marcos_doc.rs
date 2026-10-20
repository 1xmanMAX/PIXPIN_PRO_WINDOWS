//! **Los marcos numerados de los documentos** (19-oct-2026), con la misma
//! logica que los «Marcos para imprimir» del plano (`pixpin-cad`,
//! `ventana.rs`), para que en todos los visores se usen igual:
//!
//! - El boton del marco del riel (o la **I**) entra y sale del modo; pulsado,
//!   azul.
//! - En el modo, **arrastrar** crea un marco (mas de 8 px de ancho y de alto;
//!   menos es un clic y no cuenta). Va al final: su numero es su sitio (1, 2,
//!   3…), y al quitar uno los demas se renumeran solos.
//! - **Supr** quita el ultimo; «Borrar los marcos» todos.
//! - **Ctrl+P** imprime todos en grupo: una hoja por marco, en su orden.
//! - Cada marco se pinta con su raya azul y su numero blanco en un cuadrito
//!   azul en la esquina de arriba a la izquierda, y abajo a la izquierda la
//!   pista. Lo impreso va sin la raya ni el numero.
//!
//! Los marcos van **en las coordenadas del documento** (las de cada visor:
//! la hoja de un PDF, el pixel de una hoja de calculo al 100 %, el pixel CSS
//! de una pagina…): se mueven con el, no con la pantalla. Se guardan al lado
//! del fichero, en `<fichero>.pixpin-marcos` (JSON).
//!
//! Imprimir: cada visor se acerca a cada marco hasta que llena la ventana, se
//! pinta sin su interfaz y se fotografia (`marco_imprimir::foto_de_pantalla`);
//! las fotos van juntas al dialogo (`marco_imprimir::imprimir_fotos`).
//!
//! Aqui tambien las **vistas de los marcadores** (`vistas`): lo que guarda un
//! marcador ademas de su sitio (el zoom; en un modelo 3D, la camara y el
//! corte), para volver justo a esa parte.

use std::path::{Path, PathBuf};

use pixpin_render::{Color, Pintor, RectF};
use serde::{Deserialize, Serialize};

/// El azul de los marcos del plano (`0x0078D4`).
pub const AZUL: Color = Color { r: 0.0, g: 0x78 as f32 / 255.0, b: 0xD4 as f32 / 255.0, a: 1.0 };
const BLANCO: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
const CRISTAL: Color = Color { r: 0.08, g: 0.08, b: 0.09, a: 0.90 };
/// Lo minimo que tiene que medir un marco en la pantalla, de ancho y de alto.
const MINIMO_PX: f64 = 8.0;

pub const VK_I: u32 = 0x49;
pub const VK_P: u32 = 0x50;
pub const VK_SUPR: u32 = 0x2E;

/// Un marco: su caja en el documento `[x0, y0, x1, y1]` (ordenada), la parte
/// del documento donde va (la hoja de un libro de calculo; 0 si no hay) y,
/// si el visor lo necesita, su vista (la camara de un modelo 3D).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marco {
    pub caja: [f64; 4],
    #[serde(default)]
    pub parte: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vista: String,
}

impl Marco {
    pub fn ancho(&self) -> f64 {
        self.caja[2] - self.caja[0]
    }
    pub fn alto(&self) -> f64 {
        self.caja[3] - self.caja[1]
    }
    pub fn centro(&self) -> [f64; 2] {
        [(self.caja[0] + self.caja[2]) / 2.0, (self.caja[1] + self.caja[3]) / 2.0]
    }
}

/// Lo que pide una tecla o un boton del modo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pedido {
    /// Entro o salio del modo (repintar).
    Alternado,
    /// Se quito un marco (repintar y guardar).
    Cambiado,
    /// Imprimir todos (Ctrl+P).
    Imprimir,
}

/// Los marcos de un documento y el modo de ponerlos.
#[derive(Debug, Clone, Default)]
pub struct Marcos {
    pub lista: Vec<Marco>,
    /// Si se estan poniendo (el boton del riel pulsado).
    pub activo: bool,
    /// El que se esta arrastrando: sus dos esquinas en el documento.
    nuevo: Option<([f64; 2], [f64; 2], u32)>,
    /// Donde se guardan (`None`: solo esta sesion).
    fichero: Option<PathBuf>,
}

impl Marcos {
    /// Los de `doc` (los que tenga a su lado), o ninguno.
    pub fn de(doc: Option<&Path>) -> Marcos {
        let fichero = doc.map(ruta);
        let lista = fichero
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str::<Vec<Marco>>(&t).ok())
            .unwrap_or_default();
        Marcos { lista, fichero, ..Marcos::default() }
    }

    /// Guarda la lista al lado del documento (sin marcos, quita el fichero).
    pub fn guardar(&self) {
        let Some(f) = &self.fichero else { return };
        if self.lista.is_empty() {
            let _ = std::fs::remove_file(f);
            return;
        }
        match serde_json::to_string_pretty(&self.lista) {
            Ok(t) => {
                if let Err(e) = std::fs::write(f, t) {
                    tracing::warn!(?e, fichero = %f.display(), "marcos: no se pudieron guardar");
                }
            }
            Err(e) => tracing::warn!(?e, "marcos: no se pudieron escribir"),
        }
    }

    /// El boton del marco o la I: entrar o salir del modo.
    pub fn alternar(&mut self) {
        self.activo = !self.activo;
        self.nuevo = None;
    }

    pub fn arrastrando(&self) -> bool {
        self.nuevo.is_some()
    }

    /// Pulsar en el documento (`p`, sus coordenadas; `parte`, su hoja). Si
    /// el modo esta puesto se queda con el clic: empieza un marco.
    pub fn pulsar(&mut self, p: [f64; 2], parte: u32) -> bool {
        if !self.activo {
            return false;
        }
        self.nuevo = Some((p, p, parte));
        true
    }

    /// Mover con el boton pulsado: la segunda esquina. `true` si hay que
    /// repintar.
    pub fn mover(&mut self, p: [f64; 2]) -> bool {
        match &mut self.nuevo {
            Some((_, b, _)) => {
                *b = p;
                true
            }
            None => false,
        }
    }

    /// Soltar: el marco queda si mide mas de 8 px de ancho y de alto en la
    /// pantalla (`px` = pixeles de pantalla por unidad del documento).
    /// Devuelve si se anadio uno (entonces se guarda).
    pub fn soltar(&mut self, p: [f64; 2], px: f64) -> bool {
        let Some((a, _, parte)) = self.nuevo.take() else { return false };
        let caja = [a[0].min(p[0]), a[1].min(p[1]), a[0].max(p[0]), a[1].max(p[1])];
        let px = px.max(1e-9);
        if (caja[2] - caja[0]) * px <= MINIMO_PX || (caja[3] - caja[1]) * px <= MINIMO_PX {
            return false;
        }
        self.lista.push(Marco { caja, parte, vista: String::new() });
        self.guardar();
        true
    }

    /// Supr: quita el ultimo.
    pub fn quitar_ultimo(&mut self) -> bool {
        let quitado = self.lista.pop().is_some();
        if quitado {
            self.guardar();
        }
        quitado
    }

    /// «Borrar los marcos».
    pub fn borrar(&mut self) {
        self.lista.clear();
        self.guardar();
    }

    /// Las teclas del modo, como en el plano: I lo alterna, Supr (en el modo)
    /// quita el ultimo y Ctrl+P imprime. `None`: la tecla no es suya.
    pub fn tecla(&mut self, vk: u32, ctrl: bool) -> Option<Pedido> {
        match (vk, ctrl) {
            (VK_P, true) => Some(Pedido::Imprimir),
            (VK_I, false) => {
                self.alternar();
                Some(Pedido::Alternado)
            }
            (VK_SUPR, false) if self.activo => {
                self.quitar_ultimo();
                Some(Pedido::Cambiado)
            }
            _ => None,
        }
    }

    /// **Pinta los marcos** de la parte `parte` con `a_pantalla` (documento →
    /// pixeles de la ventana): la raya azul, el numero en su cuadrito y el
    /// que se esta arrastrando. Con el modo puesto, la pista abajo a la
    /// izquierda (`alto` de la ventana).
    pub fn pintar(&self, p: &Pintor, parte: u32, a_pantalla: &dyn Fn([f64; 2]) -> (f32, f32), e: f32, alto: f32, pista: &str) {
        for (i, m) in self.lista.iter().enumerate() {
            if m.parte != parte {
                continue;
            }
            let r = caja_en_pantalla(m.caja, a_pantalla);
            p.trazar(r, 2.0 * e, AZUL);
            insignia(p, r.x, r.y, i + 1, e);
        }
        if let Some((a, b, pa)) = self.nuevo
            && pa == parte
        {
            let caja = [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])];
            p.trazar(caja_en_pantalla(caja, a_pantalla), 1.5 * e, AZUL);
        }
        if self.activo {
            let tam = 12.0 * e;
            let (w, h) = p.medir_texto(pista, tam);
            let caja = RectF { x: 12.0 * e, y: alto - 34.0 * e - h / 2.0, ancho: w + 24.0 * e, alto: h + 12.0 * e };
            p.rellenar_redondeado(caja, caja.alto / 2.0, CRISTAL);
            p.texto(pista, caja.x + 12.0 * e, caja.y + 6.0 * e, tam, BLANCO);
        }
    }
}

/// **El numero de un marco**: blanco en su cuadrito azul, con la esquina en
/// `(x, y)` (la de arriba a la izquierda del marco), como en el plano.
pub fn insignia(p: &Pintor, x: f32, y: f32, n: usize, e: f32) {
    let n = n.to_string();
    let tam = 12.0 * e;
    let (w, _) = p.medir_texto(&n, tam);
    p.rellenar(RectF { x, y, ancho: w + 12.0 * e, alto: 20.0 * e }, AZUL);
    p.texto(&n, x + 6.0 * e, y + 3.0 * e, tam, BLANCO);
}

/// La caja de un marco en la ventana.
pub fn caja_en_pantalla(caja: [f64; 4], a_pantalla: &dyn Fn([f64; 2]) -> (f32, f32)) -> RectF {
    let (x0, y0) = a_pantalla([caja[0], caja[1]]);
    let (x1, y1) = a_pantalla([caja[2], caja[3]]);
    RectF { x: x0.min(x1), y: y0.min(y1), ancho: (x1 - x0).abs(), alto: (y1 - y0).abs() }
}

/// **Cuanto acercarse para que el marco llene la ventana** (con un poco de
/// aire): pixeles de pantalla por unidad del documento. Asi la foto de cada
/// marco sale con todos los pixeles que caben.
pub fn px_para_llenar(m: &Marco, ancho: f32, alto: f32) -> f64 {
    let aire = 0.94;
    ((ancho as f64 * aire) / m.ancho().max(1e-9)).min((alto as f64 * aire) / m.alto().max(1e-9))
}

/// Donde se guardan los marcos de `doc`.
pub fn ruta(doc: &Path) -> PathBuf {
    let mut s = doc.as_os_str().to_owned();
    s.push(".pixpin-marcos");
    PathBuf::from(s)
}

/// El texto de la pista, del catalogo.
pub fn pista(textos: &pixpin_store::Catalogo) -> String {
    textos.t("marcos-pista")
}

// ---------------------------------------------------------------------------
// Las vistas de los marcadores

/// **Las vistas de los marcadores** de un documento: por cada marcador (su
/// id), lo que hace falta para volver justo ahi ademas de su sitio (el zoom,
/// o la camara y el estado de un modelo 3D). Se guardan al lado del fichero,
/// en `<fichero>.pixpin-vistas` (JSON id → texto del visor), para no tocar el
/// `.pixpin-lectura` que tambien leen el movil y las versiones de antes.
#[derive(Debug, Clone, Default)]
pub struct Vistas {
    pub de: std::collections::BTreeMap<String, String>,
    fichero: Option<PathBuf>,
}

impl Vistas {
    pub fn de_documento(doc: Option<&Path>) -> Vistas {
        let fichero = doc.map(|d| {
            let mut s = d.as_os_str().to_owned();
            s.push(".pixpin-vistas");
            PathBuf::from(s)
        });
        let de = fichero
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Vistas { de, fichero }
    }

    pub fn poner(&mut self, id: &str, vista: String) {
        self.de.insert(id.to_string(), vista);
        self.guardar();
    }

    pub fn quitar(&mut self, id: &str) {
        if self.de.remove(id).is_some() {
            self.guardar();
        }
    }

    pub fn de_marca(&self, id: &str) -> Option<&str> {
        self.de.get(id).map(String::as_str)
    }

    fn guardar(&self) {
        let Some(f) = &self.fichero else { return };
        if self.de.is_empty() {
            let _ = std::fs::remove_file(f);
            return;
        }
        if let Ok(t) = serde_json::to_string_pretty(&self.de) {
            let _ = std::fs::write(f, t);
        }
    }
}

/// Una vista sencilla «zoom x y» (la de casi todos los visores) a texto.
pub fn zoom_a_texto(zoom: f64, x: f64, y: f64) -> String {
    format!("{zoom} {x} {y}")
}

/// Y de vuelta: `(zoom, x, y)`.
pub fn zoom_de_texto(t: &str) -> Option<(f64, f64, f64)> {
    let mut v = t.split_whitespace().map(|s| s.parse::<f64>().ok());
    Some((v.next()??, v.next()??, v.next()??))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn dir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-marcos-{n}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn como_en_el_plano_se_numeran_por_orden_y_supr_quita_el_ultimo() {
        let mut m = Marcos::default();
        assert!(!m.pulsar([0.0, 0.0], 0), "sin el modo, el clic es del visor");
        m.alternar();
        for i in 0..3 {
            let x = i as f64 * 100.0;
            assert!(m.pulsar([x, 0.0], 0));
            m.mover([x + 50.0, 40.0]);
            assert!(m.soltar([x + 50.0, 40.0], 1.0));
        }
        assert_eq!(m.lista.len(), 3);
        assert_eq!(m.lista[2].caja, [200.0, 0.0, 250.0, 40.0]);
        assert_eq!(m.tecla(VK_SUPR, false), Some(Pedido::Cambiado));
        assert_eq!(m.lista.len(), 2);
        assert_eq!(m.tecla(VK_P, true), Some(Pedido::Imprimir));
        assert_eq!(m.tecla(VK_I, false), Some(Pedido::Alternado));
        assert!(!m.activo);
        // Fuera del modo, Supr no es suyo (borra lo del visor).
        assert_eq!(m.tecla(VK_SUPR, false), None);
    }

    #[test]
    fn un_clic_no_es_un_marco_y_se_ordena_arrastre_hacia_donde_arrastre() {
        let mut m = Marcos::default();
        m.alternar();
        m.pulsar([10.0, 10.0], 0);
        assert!(!m.soltar([12.0, 30.0], 1.0), "2 px de ancho es un clic");
        // A doble zoom, 5 unidades son 10 px: si cuenta.
        m.pulsar([50.0, 50.0], 0);
        assert!(m.soltar([45.0, 40.0], 2.0));
        assert_eq!(m.lista[0].caja, [45.0, 40.0, 50.0, 50.0]);
    }

    #[test]
    fn se_guardan_al_lado_del_fichero_y_vuelven() {
        let d = dir("guardar");
        let doc = d.join("plano.pdf");
        let mut m = Marcos::de(Some(&doc));
        m.alternar();
        m.pulsar([0.0, 0.0], 2);
        m.soltar([100.0, 80.0], 1.0);
        assert!(ruta(&doc).is_file());
        let otra = Marcos::de(Some(&doc));
        assert_eq!(otra.lista, m.lista);
        assert_eq!(otra.lista[0].parte, 2);
        m.borrar();
        assert!(!ruta(&doc).exists(), "sin marcos no queda fichero");
    }

    #[test]
    fn la_vista_de_un_marcador_vuelve_igual() {
        let d = dir("vistas");
        let doc = d.join("hoja.xlsx");
        let mut v = Vistas::de_documento(Some(&doc));
        v.poner("a1", zoom_a_texto(1.75, 120.0, 3400.5));
        let otra = Vistas::de_documento(Some(&doc));
        assert_eq!(otra.de_marca("a1").and_then(zoom_de_texto), Some((1.75, 120.0, 3400.5)));
        assert_eq!(zoom_de_texto("roto"), None);
    }

    #[test]
    fn para_llenar_la_ventana_manda_el_lado_que_mas_aprieta() {
        let m = Marco { caja: [0.0, 0.0, 200.0, 100.0], parte: 0, vista: String::new() };
        let px = px_para_llenar(&m, 1000.0, 1000.0);
        assert!((px - 4.7).abs() < 1e-6, "{px}");
    }
}
