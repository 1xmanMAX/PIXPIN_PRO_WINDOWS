//! **El riel de la hoja de calculo** (19-oct-2026): el mismo de los PDF y
//! los Word (`pixpin_ui::riel_marcas::Riel`, pintado con
//! `lector::pintar_riel`) a la derecha, con sus tres botones y los mismos
//! iconos (el usuario: «el lapiz, el marcador y el marco deben ser
//! universales… los mismos iconos en todos los visores para no
//! confundir»):
//!
//! - **El marcador**: abre la tira de emoticonos y pone una marca en la celda
//!   activa (si se ve; si no, en la de arriba a la izquierda de lo que se
//!   ve). Cada marca es un punto del riel; un clic lleva a su hoja y a su
//!   celda, arriba a la izquierda, y el derecho la quita. Se guardan junto al
//!   libro (`<libro>.pixpin-lectura`, la linea `marcas` del PDF), con `x` la
//!   columna e `y` la hoja por [`POR_HOJA`] mas la fila: numeros enteros, que
//!   la linea redondea a centesimas, y el orden del riel sale solo (hoja,
//!   fila, columna).
//! - **El lapiz**: anotar encima de las celdas con las herramientas del
//!   lienzo (`lector_tinta::Tinta`, ver `capa`); pulsado mientras se anota.
//!   El boton «Editar» de la barra del pin sigue siendo el de escribir en las
//!   celdas.
//! - **El marco**: los marcos numerados para imprimir, como en el plano
//!   (`marcos`): pulsado, azul, mientras se ponen.
//!
//! Cada marca guarda ademas su vista (el zoom y el desplazamiento exactos,
//! `marcos_doc::Vistas`, en `<libro>.pixpin-vistas` por su id): ir a ella
//! vuelve justo a lo que se veia. Las de antes (sin vista), a su celda.
//!
//! El riel va debajo de la barra de formulas y de las letras de las columnas
//! y acaba encima de las pestanas: no tapa la barra de desplazamiento (que va
//! pegada al borde) ni los encabezados.

use std::path::{Path, PathBuf};

use pixpin_motor2d::marcas::{self, Marca};
use pixpin_render::hoja_office;
use pixpin_render::{MotorRender, Pintor};
use pixpin_shell::overlay::{EventoOverlay, VentanaOverlay};
use pixpin_store::Catalogo;
use pixpin_ui::riel_marcas::{DestinoRiel, Riel};

use super::{Estado, Seleccion, VK_E, VK_Y, VK_Z, acotar, dentro, logica, vista_celdas, vista_de};
use crate::lector;

/// Cuanto vale una hoja en la `y` de una marca: mas filas de las que caben
/// en un Excel (1 048 576).
pub(super) const POR_HOJA: f64 = 10_000_000.0;

/// Una marca en la fila `f` y la columna `c` de la hoja `hoja`.
pub(super) fn sitio_de_marca(hoja: usize, (f, c): (u32, u32)) -> (f64, f64) {
    (c as f64, hoja as f64 * POR_HOJA + f as f64)
}

/// De que hoja, fila y columna es una marca.
pub(super) fn celda_de_marca(m: &Marca) -> (usize, (u32, u32)) {
    let y = m.y.max(0.0).round();
    let hoja = (y / POR_HOJA).floor();
    let fila = (y - hoja * POR_HOJA).max(0.0);
    (hoja as usize, (fila as u32, m.x.max(0.0).round() as u32))
}

/// Lo que se recuerda del libro (sus marcas): junto a el, si hay fichero.
pub(super) struct Recuerdo {
    ruta: Option<PathBuf>,
    ajustes: pixpin_docs::lectura::Ajustes,
}

impl Recuerdo {
    pub(super) fn de(ruta: Option<&Path>) -> (Recuerdo, Vec<Marca>) {
        let ajustes = ruta.map(pixpin_docs::lectura::leer).unwrap_or_default();
        let lista = marcas::de_texto(&ajustes.marcas);
        (
            Recuerdo {
                ruta: ruta.map(Path::to_path_buf),
                ajustes,
            },
            lista,
        )
    }

    /// Sin fichero (una tabla del chat sin libro) las marcas son de la sesion.
    fn guardar(&mut self, lista: &[Marca]) {
        let Some(r) = self.ruta.as_deref() else {
            return;
        };
        self.ajustes.marcas = marcas::a_texto(lista);
        if let Err(e) = pixpin_docs::lectura::escribir(r, &self.ajustes) {
            tracing::info!(?e, "visor de hojas: no se pudieron guardar las marcas");
        }
    }
}

/// Baja todo el riel `dy` pixeles.
fn bajar(r: &mut Riel, dy: i32) {
    for c in [
        &mut r.isla,
        &mut r.poner_marca,
        &mut r.hojita,
        &mut r.imprimir,
        &mut r.columna,
    ] {
        c.y += dy;
    }
    for p in r.puntos.iter_mut() {
        p.y += dy;
    }
    if let Some(t) = r.tira.as_mut() {
        t.marco.y += dy;
        t.cerrar.y += dy;
        for c in t.celdas.iter_mut() {
            c.y += dy;
        }
    }
}

impl Estado<'_> {
    pub(super) fn escala_por_cien(&self) -> u32 {
        (self.e * 100.0).round() as u32
    }

    /// **El riel, colocado**: a la derecha, de debajo de las letras de las
    /// columnas a encima de las pestanas. La tira de emoticonos, debajo de
    /// las letras (o de la barra del lapiz, si se anota).
    pub(super) fn riel(&self) -> Riel {
        let e = self.e;
        let celdas = match self.hojas.get(self.activa).and_then(Option::as_ref) {
            Some(ab) => vista_de(ab, self.zonas().hoja).1,
            None => self.zonas().hoja,
        };
        // La isla, 8 px por debajo de las letras (el riel deja 16 arriba).
        let dy = (celdas.y - 8.0 * e).max(0.0).round() as i32;
        let fondo = (celdas.y + celdas.alto).max(dy as f32 + 1.0);
        let bajo = if self.anotando {
            celdas.y.max(60.0 * e)
        } else {
            celdas.y
        };
        let mut r = Riel::colocar(
            self.ancho.max(1.0) as u32,
            (fondo - dy as f32) as u32,
            self.escala_por_cien(),
            self.marcas.len(),
            self.poniendo_marca,
            lector::EMOJIS.len() + 1,
            bajo as i32 - dy,
        );
        bajar(&mut r, dy);
        r
    }

    pub(super) fn destino_del_riel(&self) -> DestinoRiel {
        self.riel()
            .destino(lector::punto(self.raton.0, self.raton.1))
    }

    /// Lo que tiene el raton encima en el riel; `true` si cambio (repintar).
    pub(super) fn mirar_riel(&mut self) -> bool {
        let d = self.destino_del_riel();
        let cambio = d != self.encima;
        self.encima = d;
        cambio
    }

    /// **Un clic en el riel**. `true` si era suyo (no le toca a la hoja).
    pub(super) fn pulsar_riel(&mut self, motor: &MotorRender, textos: &Catalogo) -> bool {
        match self.destino_del_riel() {
            DestinoRiel::PonerMarca => self.poniendo_marca = !self.poniendo_marca,
            DestinoRiel::Hojita => self.alternar_anotar(motor),
            DestinoRiel::Imprimir => self.alternar_marcos(motor),
            DestinoRiel::Marca(i) => self.ir_a_la_marca(i, motor, textos),
            DestinoRiel::Emoji(i) => self.poner_marca(i),
            DestinoRiel::CerrarTira => self.poniendo_marca = false,
            DestinoRiel::Hueco => {}
            DestinoRiel::Fuera => {
                // Con la tira abierta, un clic fuera solo la cierra (como el PDF).
                if self.poniendo_marca {
                    self.poniendo_marca = false;
                    return true;
                }
                return false;
            }
        }
        true
    }

    /// El clic derecho en un punto del riel quita su marca.
    pub(super) fn pulsar_derecho_riel(&mut self) -> bool {
        let DestinoRiel::Marca(i) = self.destino_del_riel() else {
            return false;
        };
        let Some(id) = self.marcas.get(i).map(|m| m.id) else {
            return false;
        };
        self.marcas = marcas::sin(&self.marcas, id);
        self.recuerdo.guardar(&self.marcas);
        self.vistas.quitar(&id.to_string());
        self.encima = self.destino_del_riel();
        true
    }

    /// **Una marca con el emoticono `i` de la tira**: en la celda activa si se
    /// ve; si no, en la de arriba a la izquierda de lo que se ve.
    pub(super) fn poner_marca(&mut self, i: usize) {
        let caja = self.zonas().hoja;
        let ab = self.abierta();
        let (v, celdas) = vista_de(ab, caja);
        let activa = ab.sel.activa;
        let se_ve = hoja_office::caja_de_celda(
            &ab.rejilla,
            &vista_celdas(&v, celdas),
            celdas,
            activa.0,
            activa.1,
        )
        .is_some_and(|[x0, y0, x1, y1]| {
            x1 > celdas.x
                && y1 > celdas.y
                && x0 < celdas.x + celdas.ancho
                && y0 < celdas.y + celdas.alto
        });
        let celda = if se_ve {
            activa
        } else {
            (ab.rejilla.fila_en(ab.y), ab.rejilla.col_en(ab.x))
        };
        let (x, y) = sitio_de_marca(self.activa, celda);
        let emoji = lector::emoji_de_la_tira(i);
        let ahora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let antes = std::mem::take(&mut self.marcas);
        self.marcas = marcas::con(&antes, x, y, emoji, ahora);
        self.poniendo_marca = false;
        self.recuerdo.guardar(&self.marcas);
        // Y lo que se ve: el zoom y el desplazamiento exactos.
        let ab = self.abierta();
        let vista = crate::marcos_doc::zoom_a_texto(ab.zoom, ab.x, ab.y);
        if let Some(nueva) = self
            .marcas
            .iter()
            .find(|m| !antes.iter().any(|a| a.id == m.id))
        {
            self.vistas.poner(&nueva.id.to_string(), vista);
        }
        // Las que se fueron (pasan de 24), sin vista.
        let quedan: Vec<String> = self.marcas.iter().map(|m| m.id.to_string()).collect();
        let sobran: Vec<String> = self
            .vistas
            .de
            .keys()
            .filter(|k| !quedan.contains(k))
            .cloned()
            .collect();
        for k in sobran {
            self.vistas.quitar(&k);
        }
    }

    /// **A la marca `i`**: su hoja, su celda elegida y lo que se veia al
    /// ponerla (el zoom y el desplazamiento); las de antes, sin vista, con
    /// su celda arriba a la izquierda.
    pub(super) fn ir_a_la_marca(&mut self, i: usize, motor: &MotorRender, textos: &Catalogo) {
        let Some(m) = self.marcas.get(i) else { return };
        let (hoja, (f, c)) = celda_de_marca(m);
        let vista = self
            .vistas
            .de_marca(&m.id.to_string())
            .and_then(crate::marcos_doc::zoom_de_texto);
        if hoja >= self.libro.hojas.len() {
            return;
        }
        if hoja != self.activa {
            self.cambiar_hoja(hoja, motor, textos);
            if self.activa != hoja {
                return;
            }
        }
        self.ver_la_marca((f, c), vista);
    }

    /// Lo de la marca en la hoja abierta: su celda elegida y su vista (o, sin
    /// vista, la celda arriba a la izquierda).
    pub(super) fn ver_la_marca(&mut self, (f, c): (u32, u32), vista: Option<(f64, f64, f64)>) {
        let caja = self.zonas().hoja;
        let ab = self.abierta_mut();
        ab.sel = Seleccion::en(f, c);
        if let Some((zoom, x, y)) = vista {
            ab.zoom = logica::acotar_zoom(zoom);
            ab.x = x;
            ab.y = y;
        } else {
            if f >= ab.rejilla.filas_fijas {
                ab.y = ab.rejilla.y(f);
            }
            if c >= ab.rejilla.columnas_fijas {
                ab.x = ab.rejilla.x(c);
            }
        }
        let (_, celdas) = vista_de(ab, caja);
        acotar(ab, celdas);
        if self.editando {
            self.sel_al_editor();
        }
    }

    /// **El lapiz del riel**: empezar o dejar de anotar. Anotar y escribir en
    /// las celdas no van a la vez: al anotar se deja de editar.
    pub(super) fn alternar_anotar(&mut self, motor: &MotorRender) {
        if self.anotando {
            self.dejar_de_anotar();
        } else {
            self.dejar_de_editar(motor);
            // Con los marcos no se anota (como en el plano): se dejan.
            if self.marcos.activo {
                self.marcos.alternar();
            }
            self.anotando = true;
            self.poniendo_marca = false;
            self.gesto = None;
        }
    }

    pub(super) fn dejar_de_anotar(&mut self) {
        if !self.anotando {
            return;
        }
        self.devolver_lo_anotado();
        self.anotando = false;
    }

    /// Lo anotado en la vista, a la capa y al disco (si cambio algo).
    fn devolver_lo_anotado(&mut self) {
        if !std::mem::take(&mut self.tinta_cambio) {
            return;
        }
        let Estado {
            libro,
            hojas,
            activa,
            anotado,
            ..
        } = self;
        let Some(ab) = hojas.get(*activa).and_then(Option::as_ref) else {
            return;
        };
        anotado.devolver(libro, &ab.hoja, &ab.rejilla);
    }

    /// Si el clic es del visor aunque se este anotando: el riel, la tira, las
    /// barras de desplazamiento y las pestanas.
    fn el_visor_primero(&self) -> bool {
        !matches!(self.destino_del_riel(), DestinoRiel::Fuera)
            || self.poniendo_marca
            || self.barra_bajo_el_raton().is_some()
            || dentro(&self.zonas().pestanas, self.raton)
    }

    /// **Anotando, la tinta primero** (como `lector_pdf`): cada evento a las
    /// herramientas del lienzo, salvo el clic en el riel, las barras o las
    /// pestanas, Ctrl+E y Escape cuando la tinta no tiene nada que soltar.
    /// `true` si el evento ya esta hecho (el visor no hace nada mas con el).
    pub(super) fn tinta_primero(
        &mut self,
        ev: &EventoOverlay,
        ventana: &mut VentanaOverlay,
        area: pixpin_geom::Rect,
    ) -> bool {
        let local = |p: pixpin_geom::Punto| ((p.x - area.x) as f32, (p.y - area.y) as f32);
        let escribiendo = self
            .anotado
            .tinta()
            .is_some_and(|t| t.gesto.esta_escribiendo());
        let para_la_tinta = match *ev {
            EventoOverlay::RatonMovido(_)
            | EventoOverlay::Muestra(_)
            | EventoOverlay::BotonSoltado(_) => self.gesto.is_none(),
            EventoOverlay::Caracter(_) => true,
            EventoOverlay::BotonPulsado(p) => {
                self.raton = local(p);
                !self.el_visor_primero()
            }
            // Ctrl+Z / Ctrl+Y: lo ultimo anotado (escribiendo un texto, el
            // deshacer es del texto).
            EventoOverlay::Tecla {
                vk: vk @ (VK_Z | VK_Y),
                ctrl: true,
                alt: false,
                shift,
            } if !escribiendo => {
                let rehacer = vk == VK_Y || shift;
                let Estado {
                    libro,
                    hojas,
                    activa,
                    anotado,
                    ..
                } = self;
                if let Some(ab) = hojas.get(*activa).and_then(Option::as_ref) {
                    anotado.deshacer_lo_mio(libro, &ab.hoja, &ab.rejilla, rehacer);
                }
                return true;
            }
            EventoOverlay::Tecla {
                vk: VK_E,
                ctrl: true,
                ..
            } => false,
            EventoOverlay::Tecla { vk, .. } => {
                vk != logica::VK_ESCAPE || self.anotado.tinta().is_some_and(|t| t.quiere_escape())
            }
            _ => false,
        };
        if !para_la_tinta {
            return false;
        }
        if let EventoOverlay::RatonMovido(p) | EventoOverlay::BotonPulsado(p) = *ev {
            self.raton = local(p);
            self.encima = self.destino_del_riel();
        }
        let escala = self.escala_por_cien();
        let caja = self.zonas().hoja;
        let Estado {
            libro,
            hojas,
            activa,
            anotado,
            ..
        } = self;
        let Some(ab) = hojas.get(*activa).and_then(Option::as_ref) else {
            return false;
        };
        let (v, celdas) = vista_de(ab, caja);
        let camara = super::capa::camara(&ab.rejilla, &v, celdas);
        let h = anotado.evento(
            ev,
            (libro, &ab.hoja, *activa),
            &ab.rejilla,
            &camara,
            (area, escala),
        );
        if let Some(c) = h.cursor {
            ventana.poner_cursor(crate::dibujo::teclas::forma_de(c));
        }
        self.tinta_cambio |= h.cambio;
        if h.arrastra {
            self.trazo = true;
            ventana.capturar_raton();
        }
        let acaba = match *ev {
            EventoOverlay::BotonSoltado(_) => {
                if std::mem::take(&mut self.trazo) {
                    ventana.soltar_raton();
                }
                true
            }
            EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_) => false,
            _ => !self.anotado.tinta().is_some_and(|t| t.trazando()),
        };
        // Un gesto acabado se guarda ya (un paso de deshacer por gesto).
        if acaba {
            self.devolver_lo_anotado();
        }
        if h.salir {
            self.dejar_de_anotar();
        }
        // El raton, mientras se anota, es solo de la tinta y del riel.
        matches!(
            *ev,
            EventoOverlay::RatonMovido(_) | EventoOverlay::Muestra(_)
        ) || h.consumido
            || h.salir
    }

    /// El gesto de pararse con el lapiz: el trazo quieto se vuelve figura.
    pub(super) fn forma_rapida(&mut self) -> bool {
        if !self.anotando {
            return false;
        }
        let zoom = self.abierta().zoom as f32;
        if !self.anotado.forma_rapida(zoom) {
            return false;
        }
        self.tinta_cambio = true;
        if !self.anotado.tinta().is_some_and(|t| t.trazando()) {
            self.devolver_lo_anotado();
        }
        true
    }

    /// Anotando y con un trazo a medias (el bucle va a 60 Hz).
    pub(super) fn trazando(&self) -> bool {
        self.anotando && self.anotado.tinta().is_some_and(|t| t.trazando())
    }
}

/// **Pinta las marcas y el riel**: cada marca, un redondel pequeno en el
/// canto izquierdo de su celda si se ve; el riel a la derecha; y anotando,
/// la barra y el panel del lienzo arriba.
pub(super) fn pintar(p: &Pintor, est: &mut Estado, textos: &Catalogo) {
    let e = est.e;
    let caja = est.zonas().hoja;
    if let Some(ab) = est.hojas.get(est.activa).and_then(Option::as_ref) {
        let (v, celdas) = vista_de(ab, caja);
        let vc = vista_celdas(&v, celdas);
        for m in &est.marcas {
            let (hoja, (f, c)) = celda_de_marca(m);
            if hoja != est.activa {
                continue;
            }
            if let Some([x0, y0, _, y1]) =
                hoja_office::caja_de_celda(&ab.rejilla, &vc, celdas, f, c)
                && x0 >= celdas.x
                && y0 >= celdas.y
            {
                let radio = (7.0 * e).min((y1 - y0) / 2.0).max(4.0 * e);
                lector::redondel(
                    p,
                    &m.emoji,
                    (x0 + radio + 1.0, y0 + (y1 - y0) / 2.0),
                    radio,
                    0.85,
                );
            }
        }
    }
    let riel = est.riel();
    let emojis: Vec<&str> = est.marcas.iter().map(|m| m.emoji.as_str()).collect();
    lector::pintar_riel_con(
        p,
        &riel,
        &emojis,
        est.encima,
        est.poniendo_marca,
        est.anotando,
        est.marcos.activo,
        &textos.t("marca-elige"),
    );
    if est.anotando {
        let (area, escala) = (est.area, est.escala_por_cien());
        est.anotado.pintar_interfaz(p, area, escala);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una marca va y vuelve por la linea del fichero (que redondea a
    /// centesimas) con su hoja, su fila y su columna exactas.
    #[test]
    fn la_marca_vuelve_a_su_celda() {
        let (x, y) = sitio_de_marca(2, (1_048_575, 16_383));
        let lista = marcas::con(&[], x, y, "⭐", 1);
        let leida = marcas::de_texto(&marcas::a_texto(&lista));
        assert_eq!(celda_de_marca(&leida[0]), (2, (1_048_575, 16_383)));
    }

    /// El orden del riel: por hoja, luego por fila y luego por columna.
    #[test]
    fn el_riel_las_ordena_por_hoja_y_fila() {
        let mut l = Vec::new();
        for (i, (h, f, c)) in [(1, 0, 0), (0, 9, 3), (0, 9, 1), (0, 2, 7)]
            .into_iter()
            .enumerate()
        {
            let (x, y) = sitio_de_marca(h, (f, c));
            l = marcas::con(&l, x, y, "🔖", i as i64 + 1);
        }
        let orden: Vec<_> = marcas::de_texto(&marcas::a_texto(&l))
            .iter()
            .map(celda_de_marca)
            .collect();
        assert_eq!(
            orden,
            vec![(0, (2, 7)), (0, (9, 1)), (0, (9, 3)), (1, (0, 0))]
        );
    }

    /// Bajar el riel lo baja entero (la isla, los botones, los puntos y la
    /// tira), sin moverlo de lado.
    #[test]
    fn bajar_el_riel_lo_baja_entero() {
        let r0 = Riel::colocar(1200, 800, 100, 3, true, 13, 60);
        let mut r = r0.clone();
        bajar(&mut r, 50);
        assert_eq!(r.isla.y, r0.isla.y + 50);
        assert_eq!(r.hojita.x, r0.hojita.x);
        assert!(
            r.puntos
                .iter()
                .zip(&r0.puntos)
                .all(|(a, b)| a.y == b.y + 50)
        );
        assert_eq!(
            r.tira.as_ref().map(|t| t.cerrar.y),
            r0.tira.as_ref().map(|t| t.cerrar.y + 50)
        );
    }
}
