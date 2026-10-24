//! **Los marcos numerados de la hoja de calculo** (19-oct-2026), con la
//! logica de los del plano (`crate::marcos_doc`): el boton del marco del riel
//! (o la **I**) entra y sale del modo; arrastrar sobre las celdas pone un
//! marco, que lleva su numero (1, 2, 3…); **Supr** quita el ultimo y
//! **Ctrl+P** los imprime todos en grupo, una hoja por marco.
//!
//! Cada marco va en los pixeles de la hoja al 100 % (los mismos que
//! `Abierta::x`/`y`) y en su hoja (`parte`, el indice en el libro): se pinta
//! solo en ella y se mueve con el desplazamiento y el zoom, como las celdas
//! del panel que se mueve. Se guardan junto al libro (`.pixpin-marcos`).
//!
//! Poner marcos, anotar con el lapiz y escribir en las celdas no van a la
//! vez (como en el plano: con los marcos, no se anota).
//!
//! **Imprimir**: por cada marco, en su orden, la hoja se acerca hasta que el
//! marco llena la ventana (sin los paneles inmovilizados, que lo taparian),
//! se pinta sin la interfaz (ni el riel, ni los marcos, ni lo elegido), se
//! espera a que este en la pantalla y se fotografia su sitio. Luego todo
//! vuelve a como estaba y las fotos van juntas al dialogo de imprimir. Sin
//! marcos, Ctrl+P imprime lo que se ve.

use std::time::{Duration, Instant};

use pixpin_codec::imagen::ImagenRgba;
use pixpin_hojas::{Rejilla, Vista};
use pixpin_render::{MotorRender, Pintor, RectF, Superficie};
use pixpin_store::Catalogo;

use super::{Abierta, Estado, Gesto, Zonas, acotar, dentro, logica, vista_celdas, vista_de};
use crate::marcos_doc::{self, Marco, Pedido};

/// **De la hoja (px al 100 %) a la ventana y de vuelta**, por el panel que
/// se mueve (el de abajo a la derecha, donde va casi todo): el mismo que
/// usa la tinta, para que los marcos y lo anotado sigan a las celdas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Mapa {
    /// La esquina del panel en la ventana.
    x0: f64,
    y0: f64,
    /// El punto de la hoja que cae en esa esquina.
    ox: f64,
    oy: f64,
    z: f64,
}

impl Mapa {
    pub(super) fn de(rejilla: &Rejilla, v: &Vista, celdas: RectF) -> Mapa {
        let z = v.escala.max(0.01);
        match rejilla.paneles(&vista_celdas(v, celdas)).last() {
            Some(p) => Mapa {
                x0: celdas.x as f64 + p.pantalla[0],
                y0: celdas.y as f64 + p.pantalla[1],
                ox: p.origen.0,
                oy: p.origen.1,
                z,
            },
            None => Mapa {
                x0: celdas.x as f64,
                y0: celdas.y as f64,
                ox: v.x,
                oy: v.y,
                z,
            },
        }
    }

    /// El de la hoja abierta en la zona `caja` de la ventana.
    pub(super) fn de_abierta(ab: &Abierta, caja: RectF) -> Mapa {
        let (v, celdas) = vista_de(ab, caja);
        Mapa::de(&ab.rejilla, &v, celdas)
    }

    pub(super) fn a_pantalla(&self, [x, y]: [f64; 2]) -> (f32, f32) {
        (
            (self.x0 + (x - self.ox) * self.z) as f32,
            (self.y0 + (y - self.oy) * self.z) as f32,
        )
    }

    pub(super) fn a_hoja(&self, (x, y): (f32, f32)) -> [f64; 2] {
        [
            self.ox + (x as f64 - self.x0) / self.z,
            self.oy + (y as f64 - self.y0) / self.z,
        ]
    }

    /// Donde empieza en la ventana el panel que se mueve.
    #[cfg(test)]
    pub(super) fn esquina(&self) -> (f32, f32) {
        (self.x0 as f32, self.y0 as f32)
    }

    /// Pixeles de pantalla por pixel de hoja (lo que mide un marco).
    pub(super) fn px(&self) -> f64 {
        self.z
    }
}

/// **Acerca la hoja hasta que el marco llena la zona de celdas**, centrado
/// (la hoja sin paneles inmovilizados: el que imprime los quita un
/// momento). Hasta el zoom maximo del visor.
pub(super) fn encuadrar(ab: &mut Abierta, caja: RectF, m: &Marco) {
    // Los encabezados cambian de ancho con el zoom: unas vueltas, hasta que
    // no se mueve.
    for _ in 0..6 {
        let (_, celdas) = vista_de(ab, caja);
        ab.zoom = logica::acotar_zoom(marcos_doc::px_para_llenar(m, celdas.ancho, celdas.alto));
    }
    let (_, celdas) = vista_de(ab, caja);
    let [cx, cy] = m.centro();
    ab.x = cx - celdas.ancho as f64 / 2.0 / ab.zoom;
    ab.y = cy - celdas.alto as f64 / 2.0 / ab.zoom;
}

/// La caja `r` (en la ventana) dentro de `limite`, en pixeles de pantalla
/// (`origen`: la esquina de la ventana en el escritorio). `None` si no se ve.
fn en_pantalla(r: RectF, limite: RectF, origen: (i32, i32)) -> Option<pixpin_geom::Rect> {
    let x0 = r.x.max(limite.x).round();
    let y0 = r.y.max(limite.y).round();
    let x1 = (r.x + r.ancho).min(limite.x + limite.ancho).round();
    let y1 = (r.y + r.alto).min(limite.y + limite.alto).round();
    (x1 - x0 >= 1.0 && y1 - y0 >= 1.0).then(|| pixpin_geom::Rect {
        x: origen.0 + x0 as i32,
        y: origen.1 + y0 as i32,
        ancho: (x1 - x0) as u32,
        alto: (y1 - y0) as u32,
    })
}

/// Las entradas del menu de los marcos.
const MENU_IMPRIMIR: u32 = 1;
const MENU_BORRAR: u32 = 2;

/// Las fotos que dejo la ultima impresion (en las pruebas no se abre el
/// dialogo: las fotos se quedan aqui).
#[cfg(test)]
pub(crate) static FOTOS: std::sync::Mutex<Vec<ImagenRgba>> = std::sync::Mutex::new(Vec::new());

impl Estado<'_> {
    /// **El boton del marco o la I**: entrar o salir del modo. Al entrar se
    /// deja de anotar, de escribir en las celdas y de elegir el emoticono.
    pub(super) fn alternar_marcos(&mut self, motor: &MotorRender) {
        if !self.marcos.activo {
            self.dejar_de_anotar();
            self.dejar_de_editar(motor);
            self.poniendo_marca = false;
        }
        self.marcos.alternar();
        if matches!(self.gesto, Some(Gesto::Marco)) {
            self.gesto = None;
        }
    }

    fn mapa(&self) -> Mapa {
        Mapa::de_abierta(self.abierta(), self.zonas().hoja)
    }

    /// Pulsar en la hoja con el modo puesto: empieza un marco (si es en las
    /// celdas). `true` si hay que capturar el raton.
    pub(super) fn pulsar_marco(&mut self) -> bool {
        let (_, celdas) = self.vista();
        if !dentro(&celdas, self.raton) {
            return false;
        }
        let p = self.mapa().a_hoja(self.raton);
        if self.marcos.pulsar(p, self.activa as u32) {
            self.gesto = Some(Gesto::Marco);
            return true;
        }
        false
    }

    /// Arrastrando: la otra esquina. `true` si hay que repintar.
    pub(super) fn mover_marco(&mut self) -> bool {
        let p = self.mapa().a_hoja(self.raton);
        self.marcos.mover(p)
    }

    /// Al soltar: el marco queda si mide mas de 8 px (se guarda solo).
    pub(super) fn soltar_marco(&mut self) -> bool {
        let m = self.mapa();
        self.marcos.soltar(m.a_hoja(self.raton), m.px())
    }

    /// Las teclas de los marcos (viendo; escribiendo en las celdas las letras
    /// y Supr son de la celda). `None`: no es suya.
    pub(super) fn tecla_de_marcos(
        &mut self,
        vk: u32,
        ctrl: bool,
        motor: &MotorRender,
    ) -> Option<Pedido> {
        if self.editando && vk != marcos_doc::VK_P {
            return None;
        }
        if vk == marcos_doc::VK_I && !ctrl {
            // Lo mismo que el boton: con lo que deja de hacer al entrar.
            self.alternar_marcos(motor);
            return Some(Pedido::Alternado);
        }
        self.marcos.tecla(vk, ctrl)
    }

    /// **Las entradas del menu de los marcos** (el clic derecho en la hoja):
    /// imprimirlos y borrarlos, solo si hay marcos (y no escribiendo en una
    /// celda), como en el menu del PDF.
    pub(super) fn entradas_de_marcos(&self, textos: &Catalogo) -> Vec<(u32, String)> {
        if self.marcos.lista.is_empty() || self.escribiendo() {
            return Vec::new();
        }
        vec![
            (MENU_IMPRIMIR, textos.t("marcos-imprimir")),
            (MENU_BORRAR, textos.t("marcos-borrar")),
        ]
    }

    /// Lo elegido en ese menu: imprimir (lo hace el bucle, con su
    /// superficie) o borrar todos ya.
    pub(super) fn elegido_de_marcos(&mut self, id: u32) -> Option<Pedido> {
        match id {
            MENU_IMPRIMIR => Some(Pedido::Imprimir),
            MENU_BORRAR => {
                self.marcos.borrar();
                if matches!(self.gesto, Some(Gesto::Marco)) {
                    self.gesto = None;
                }
                Some(Pedido::Cambiado)
            }
            _ => None,
        }
    }

    /// **El menu de los marcos** junto al raton. `None` si no sale (sin
    /// marcos) o no se elige nada.
    pub(super) fn menu_de_marcos(&mut self, textos: &Catalogo) -> Option<Pedido> {
        let entradas = self.entradas_de_marcos(textos);
        if entradas.is_empty() {
            return None;
        }
        let id = pixpin_shell::menu_llano(
            windows::Win32::Foundation::HWND(self.hwnd as *mut _),
            &entradas,
        )?;
        self.elegido_de_marcos(id)
    }

    /// **Pinta los marcos** de la hoja abierta (recortados a las celdas) y,
    /// con el modo puesto, la pista encima de las pestanas.
    pub(super) fn pintar_marcos(&self, p: &Pintor, z: &Zonas, textos: &Catalogo) {
        let Some(ab) = self.hojas.get(self.activa).and_then(Option::as_ref) else {
            return;
        };
        let (v, celdas) = vista_de(ab, z.hoja);
        let mapa = Mapa::de(&ab.rejilla, &v, celdas);
        let a_pantalla = |q: [f64; 2]| mapa.a_pantalla(q);
        let pista = marcos_doc::pista(textos);
        // Los marcos y el que se arrastra, sin salirse de las celdas; la
        // pista aparte (va sobre los numeros de las filas).
        let mut sin_pista = self.marcos.clone();
        sin_pista.activo = false;
        p.con_recorte(celdas, |p| {
            sin_pista.pintar(
                p,
                self.activa as u32,
                &a_pantalla,
                self.e,
                z.pestanas.y,
                &pista,
            )
        });
        if self.marcos.activo {
            // Las fichas 1, 2, 3… (de todas las hojas) y la pista, sin los
            // marcos: con una parte que no es de ninguna hoja solo se pintan
            // ellas.
            self.marcos
                .pintar(p, NINGUNA_HOJA, &a_pantalla, self.e, z.pestanas.y, &pista);
        }
    }

    /// **La ficha de marco bajo el raton** (las de abajo a la izquierda, con
    /// el modo puesto): el marco al que lleva, desde cero.
    pub(super) fn ficha_bajo_el_raton(&self) -> Option<usize> {
        if !self.marcos.activo {
            return None;
        }
        self.marcos
            .ficha_en(self.raton, self.zonas().pestanas.y, self.e)
    }

    /// **Ir al marco `k`** (su ficha, como en el PDF y el modelo 3D): a su
    /// hoja si es de otra, y la hoja se acerca hasta que el marco llena las
    /// celdas que se mueven, centrado en ellas (lo que dejan los paneles
    /// inmovilizados, que siguen puestos).
    pub(super) fn ir_al_marco(&mut self, k: usize, motor: &MotorRender, textos: &Catalogo) {
        let Some(m) = self.marcos.lista.get(k).cloned() else {
            return;
        };
        let hoja = m.parte as usize;
        if hoja >= self.libro.hojas.len() {
            return;
        }
        if hoja != self.activa {
            self.cambiar_hoja(hoja, motor, textos);
            if self.activa != hoja {
                return;
            }
        }
        let caja = self.zonas().hoja;
        encuadrar_en_el_panel(self.abierta_mut(), caja, &m);
        if self.editando {
            self.sel_al_editor();
        }
    }
}

/// La parte que no es ninguna hoja: pintar los marcos con ella deja solo las
/// fichas y la pista.
const NINGUNA_HOJA: u32 = u32::MAX;

/// **El panel que se mueve en la ventana** (el de abajo a la derecha): de
/// donde acaban los paneles inmovilizados al final de las celdas.
fn panel_que_se_mueve(mapa: &Mapa, celdas: RectF) -> RectF {
    let (x0, y0) = (mapa.x0 as f32, mapa.y0 as f32);
    RectF {
        x: x0,
        y: y0,
        ancho: (celdas.x + celdas.ancho - x0).max(1.0),
        alto: (celdas.y + celdas.alto - y0).max(1.0),
    }
}

/// Como [`encuadrar`], pero con los paneles inmovilizados puestos: el marco
/// llena el panel que se mueve y va centrado en el. Los paneles fijos crecen
/// con el zoom (y el que se mueve encoge), asi que el zoom sale de la cuenta
/// entera, `z * (marco + fijo) <= celdas` (con el aire de `px_para_llenar`),
/// y el desplazamiento pone el centro del marco en el del panel. Unas
/// vueltas, porque los encabezados tambien cambian de ancho con el zoom.
pub(super) fn encuadrar_en_el_panel(ab: &mut Abierta, caja: RectF, m: &Marco) {
    let [cx, cy] = m.centro();
    let aire = 0.94;
    for _ in 0..6 {
        let (_, celdas) = vista_de(ab, caja);
        let fijo = (
            ab.rejilla.x(ab.rejilla.columnas_fijas),
            ab.rejilla.y(ab.rejilla.filas_fijas),
        );
        let zx = aire * celdas.ancho as f64 / (m.ancho().max(1e-9) + aire * fijo.0);
        let zy = aire * celdas.alto as f64 / (m.alto().max(1e-9) + aire * fijo.1);
        ab.zoom = logica::acotar_zoom(zx.min(zy));
        let (v, celdas) = vista_de(ab, caja);
        let mapa = Mapa::de(&ab.rejilla, &v, celdas);
        let panel = panel_que_se_mueve(&mapa, celdas);
        // El punto de la hoja en la esquina del panel, para que el centro
        // del marco caiga en el medio del panel.
        ab.x = cx - (panel.x + panel.ancho / 2.0 - mapa.x0 as f32) as f64 / ab.zoom;
        ab.y = cy - (panel.y + panel.alto / 2.0 - mapa.y0 as f32) as f64 / ab.zoom;
    }
    let (_, celdas) = vista_de(ab, caja);
    acotar(ab, celdas);
}

/// **Imprime los marcos en grupo** (Ctrl+P): las fotos de cada uno, como
/// dice el modulo, al dialogo de imprimir. Sin marcos, lo que se ve.
pub(super) fn imprimir(
    superficie: &Superficie,
    motor: &MotorRender,
    est: &mut Estado,
    textos: &Catalogo,
) {
    let Some((hwnd, cliente)) = pixpin_shell::overlay::area_cliente(Some(est.hwnd)) else {
        return;
    };
    let t = Instant::now();
    let fotos = fotos(superficie, motor, est, textos, (cliente.x, cliente.y));
    tracing::info!(
        fotos = fotos.len(),
        marcos = est.marcos.lista.len(),
        ms = t.elapsed().as_millis() as u64,
        "visor de hojas: marcos fotografiados"
    );
    if fotos.is_empty() {
        return;
    }
    #[cfg(test)]
    {
        let _ = (hwnd, &est.nombre);
        *FOTOS.lock().unwrap_or_else(|e| e.into_inner()) = fotos;
    }
    #[cfg(not(test))]
    if let Err(e) = crate::marco_imprimir::imprimir_fotos(hwnd, fotos, &est.nombre) {
        tracing::warn!(?e, "visor de hojas: no se pudieron imprimir los marcos");
    }
}

/// Pinta ya sin interfaz, espera a que se vea y fotografia `caja` (en la
/// ventana) dentro de `limite`.
fn foto(
    superficie: &Superficie,
    motor: &MotorRender,
    est: &mut Estado,
    textos: &Catalogo,
    origen: (i32, i32),
    que: impl Fn(&Estado) -> Option<(RectF, RectF)>,
) -> Option<ImagenRgba> {
    super::pintar_ya(superficie, motor, est, textos);
    pixpin_shell::overlay::esperar_composicion();
    let (r, limite) = que(est)?;
    crate::marco_imprimir::foto_de_pantalla(en_pantalla(r, limite, origen)?)
}

fn fotos(
    superficie: &Superficie,
    motor: &MotorRender,
    est: &mut Estado,
    textos: &Catalogo,
    origen: (i32, i32),
) -> Vec<ImagenRgba> {
    let caja = est.zonas().hoja;
    est.sin_interfaz = true;
    let mut fotos = Vec::new();
    if est.marcos.lista.is_empty() {
        // Sin marcos: lo que se ve de las celdas.
        if let Some(f) = foto(superficie, motor, est, textos, origen, |est| {
            let (_, celdas) = est.vista();
            Some((celdas, celdas))
        }) {
            fotos.push(f);
        }
        est.sin_interfaz = false;
        super::pintar_ya(superficie, motor, est, textos);
        return fotos;
    }
    // Como estaba todo, para volver.
    let (activa, desde_pestana) = (est.activa, est.desde_pestana);
    let mut antes: Vec<(usize, f64, f64, f64, u32, u32)> = Vec::new();
    let lista = est.marcos.lista.clone();
    for m in &lista {
        let i = m.parte as usize;
        if i >= est.libro.hojas.len() {
            continue;
        }
        if i != est.activa {
            est.ir_a_hoja(i, motor);
            if est.activa != i {
                continue;
            }
            esperar_lo_leido(est);
        }
        let ab = est.abierta_mut();
        if !antes.iter().any(|a| a.0 == i) {
            antes.push((
                i,
                ab.x,
                ab.y,
                ab.zoom,
                ab.rejilla.filas_fijas,
                ab.rejilla.columnas_fijas,
            ));
        }
        // Sin paneles inmovilizados: el marco entero en el panel que se mueve.
        ab.rejilla.filas_fijas = 0;
        ab.rejilla.columnas_fijas = 0;
        encuadrar(ab, caja, m);
        let caja_m = m.caja;
        if let Some(f) = foto(superficie, motor, est, textos, origen, |est| {
            let ab = est.abierta();
            let (v, celdas) = vista_de(ab, caja);
            let mapa = Mapa::de(&ab.rejilla, &v, celdas);
            Some((
                marcos_doc::caja_en_pantalla(caja_m, &|q| mapa.a_pantalla(q)),
                celdas,
            ))
        }) {
            fotos.push(f);
        }
    }
    for (i, x, y, zoom, ff, cf) in antes {
        if let Some(ab) = est.hojas.get_mut(i).and_then(Option::as_mut) {
            ab.rejilla.filas_fijas = ff;
            ab.rejilla.columnas_fijas = cf;
            (ab.x, ab.y, ab.zoom) = (x, y, zoom);
            let (_, celdas) = vista_de(ab, caja);
            acotar(ab, celdas);
        }
    }
    est.activa = activa;
    est.desde_pestana = desde_pestana;
    est.sin_interfaz = false;
    super::pintar_ya(superficie, motor, est, textos);
    fotos
}

/// Una hoja recien abierta para fotografiar un marco: un poco de tiempo para
/// que llegue lo que falta por leer (como mucho 3 s).
fn esperar_lo_leido(est: &mut Estado) {
    let t = Instant::now();
    while est.abierta().tandas.is_some() && t.elapsed() < Duration::from_secs(3) {
        est.recibir();
        std::thread::sleep(Duration::from_millis(10));
    }
    est.recibir();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::marcos_doc::Marcos;

    #[test]
    fn de_la_hoja_a_la_ventana_y_de_vuelta_es_lo_mismo() {
        let m = Mapa {
            x0: 40.0,
            y0: 60.0,
            ox: 1000.0,
            oy: 500.0,
            z: 1.5,
        };
        let p = m.a_pantalla([1100.0, 520.0]);
        assert_eq!(p, (190.0, 90.0));
        let q = m.a_hoja(p);
        assert!(
            (q[0] - 1100.0).abs() < 1e-3 && (q[1] - 520.0).abs() < 1e-3,
            "{q:?}"
        );
    }

    /// El menu de los marcos sale solo con marcos, y «Borrar los marcos» los
    /// quita todos (tambien del fichero).
    #[test]
    fn el_menu_de_los_marcos_imprime_y_borra() {
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let dir =
            std::env::temp_dir().join(format!("pixpin-hoja-menu-marcos-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("libro.xlsx");
        let mut libro = pixpin_hojas::Libro::desde_bytes(
            pixpin_proyecto::tabla_xlsx::a_xlsx(
                &[pixpin_proyecto::tabla_xlsx::HojaParaVer {
                    nombre: "H",
                    tabla: &Default::default(),
                    valores: None,
                }],
                0,
            )
            .unwrap(),
            pixpin_hojas::Regional::es_pe(),
        )
        .unwrap();
        let mut est = super::super::pruebas::estado_de_prueba(&mut libro);
        // Caso negativo: sin marcos no hay menu.
        assert!(est.entradas_de_marcos(&textos).is_empty());
        est.marcos = Marcos::de(Some(&doc));
        est.marcos.alternar();
        est.marcos.pulsar([0.0, 0.0], 0);
        assert!(est.marcos.soltar([100.0, 80.0], 1.0));
        let e = est.entradas_de_marcos(&textos);
        assert_eq!(
            e.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
            ["Imprimir los marcos (Ctrl+P)", "Borrar los marcos"]
        );
        assert_eq!(est.elegido_de_marcos(e[0].0), Some(Pedido::Imprimir));
        assert_eq!(est.marcos.lista.len(), 1, "imprimir no quita nada");
        assert!(marcos_doc::ruta(&doc).is_file());
        assert_eq!(est.elegido_de_marcos(e[1].0), Some(Pedido::Cambiado));
        assert!(est.marcos.lista.is_empty());
        assert!(
            !marcos_doc::ruta(&doc).exists(),
            "borrados tambien del fichero"
        );
        assert!(est.entradas_de_marcos(&textos).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lo_que_no_se_ve_no_se_fotografia() {
        let limite = RectF {
            x: 50.0,
            y: 30.0,
            ancho: 500.0,
            alto: 400.0,
        };
        let r = en_pantalla(
            RectF {
                x: 0.0,
                y: 0.0,
                ancho: 100.0,
                alto: 100.0,
            },
            limite,
            (1000, 200),
        )
        .unwrap();
        assert_eq!(
            r,
            pixpin_geom::Rect {
                x: 1050,
                y: 230,
                ancho: 50,
                alto: 70
            },
            "recortado a las celdas"
        );
        // Caso negativo: del todo fuera.
        assert!(
            en_pantalla(
                RectF {
                    x: 600.0,
                    y: 0.0,
                    ancho: 10.0,
                    alto: 10.0
                },
                limite,
                (0, 0)
            )
            .is_none()
        );
    }
}
