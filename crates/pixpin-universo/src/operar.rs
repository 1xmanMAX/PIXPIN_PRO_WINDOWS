//! Lo que el usuario hace con los astros. Cada operacion es UN paso de
//! deshacer, y si una regla la rechaza no deja nada a medias.

use pixpin_motor2d::Escena;

use crate::astro::{Astro, Clase, Conexion, IdAstro, RADIO_LUNA, RADIO_PLANETA_S, TipoConexion};
use crate::jerarquia::{Rechazo, aterrizaje, descendientes};
use crate::universo::Universo;

/// Un arrastre en curso. Lo que se mueve se decide al empezar y no cambia.
#[derive(Debug, Clone)]
pub struct Arrastre {
    /// Los elegidos sin los que ya van dentro de otro elegido.
    raices: Vec<IdAstro>,
    /// Raices y descendientes, con su posicion de partida.
    origen: Vec<(IdAstro, f32, f32)>,
    anotaciones: Vec<u64>,
    hecho: (f32, f32),
    propio: bool,
}

impl Arrastre {
    pub fn ids(&self) -> impl Iterator<Item = IdAstro> + '_ {
        self.origen.iter().map(|(id, _, _)| *id)
    }
}

impl Universo {
    /// D204: lo que cae entero dentro del circulo de un contenedor.
    pub fn anotaciones_dentro(&self, id: IdAstro, escena: &Escena) -> Vec<u64> {
        let Some(a) = self.astro(id).filter(|a| a.es_contenedor()) else {
            return Vec::new();
        };
        escena
            .visibles()
            .filter(|e| a.contiene_caja(e.caja()))
            .map(|e| e.id)
            .collect()
    }

    pub fn empezar_arrastre(&mut self, elegidos: &[IdAstro], escena: &Escena) -> Arrastre {
        let raices: Vec<IdAstro> = elegidos
            .iter()
            .copied()
            .filter(|id| {
                // Fuera los que ya arrastra un antepasado elegido.
                let mut p = self.astro(*id).and_then(|a| a.padre);
                while let Some(x) = p {
                    if elegidos.contains(&x) {
                        return false;
                    }
                    p = self.astro(x).and_then(|a| a.padre);
                }
                self.astro(*id).is_some()
            })
            .collect();
        let mut ids: Vec<IdAstro> = Vec::new();
        for r in &raices {
            if !ids.contains(r) {
                ids.push(*r);
            }
            for d in descendientes(&self.astros, *r) {
                if !ids.contains(&d) {
                    ids.push(d);
                }
            }
        }
        let mut anotaciones: Vec<u64> = Vec::new();
        for id in &ids {
            for e in self.anotaciones_dentro(*id, escena) {
                if !anotaciones.contains(&e) {
                    anotaciones.push(e);
                }
            }
        }
        let propio = self.empezar();
        for id in &ids {
            self.apuntar_astro(*id);
        }
        let origen = ids
            .iter()
            .filter_map(|id| self.astro(*id).map(|a| (*id, a.x, a.y)))
            .collect();
        Arrastre {
            raices,
            origen,
            anotaciones,
            hecho: (0.0, 0.0),
            propio,
        }
    }

    /// `dx`, `dy`: desplazamiento TOTAL desde que empezo el arrastre. Los
    /// astros se ponen en origen + total (sin acumular error); las
    /// anotaciones se mueven lo que falta, que en una suma es exacto.
    pub fn arrastrar(&mut self, a: &mut Arrastre, dx: f32, dy: f32, escena: &mut Escena) {
        for (id, ox, oy) in &a.origen {
            if let Some(x) = self.astros.iter_mut().find(|x| x.id == *id) {
                x.x = ox + dx;
                x.y = oy + dy;
            }
        }
        let (px, py) = (dx - a.hecho.0, dy - a.hecho.1);
        for e in &a.anotaciones {
            escena.mover(*e, px, py);
        }
        a.hecho = (dx, dy);
        self.cambios += 1;
    }

    /// Cierra el arrastre. Si una regla lo rechaza, todo vuelve a su sitio y
    /// no queda paso. `Ok(true)` = aceptado con aviso (D203).
    pub fn soltar(&mut self, a: Arrastre, escena: &mut Escena) -> Result<bool, Rechazo> {
        let ignorar: Vec<IdAstro> = a.ids().collect();
        let mut padres = Vec::new();
        let mut aviso = false;
        for r in &a.raices {
            let movido = self.astro(*r).cloned().ok_or(Rechazo::NoExiste)?;
            match aterrizaje(&self.astros, &movido, movido.x, movido.y, &ignorar) {
                Ok(t) => {
                    aviso |= t.aviso;
                    padres.push((*r, t.padre));
                }
                Err(e) => {
                    for id in &a.anotaciones {
                        escena.mover(*id, -a.hecho.0, -a.hecho.1);
                    }
                    self.cancelar(a.propio);
                    return Err(e);
                }
            }
        }
        for (id, padre) in padres {
            if let Some(x) = self.astros.iter_mut().find(|x| x.id == id) {
                x.padre = padre;
            }
        }
        if !a.anotaciones.is_empty() && a.hecho != (0.0, 0.0) {
            escena.abrir_paso();
            for id in &a.anotaciones {
                escena.apuntar_movimiento(*id, a.hecho.0, a.hecho.1);
            }
            escena.cerrar_paso();
            self.marcar_anotaciones(escena);
        }
        self.terminar(a.propio);
        Ok(aviso)
    }

    /// Saca una luna de la nebulosa al cielo (D228).
    pub fn colocar_luna(
        &mut self,
        codigo: &str,
        proyecto: &str,
        x: f32,
        y: f32,
    ) -> Result<IdAstro, Rechazo> {
        if self.luna_de(codigo).is_some() {
            return Err(Rechazo::YaColocada);
        }
        let id = self.nuevo_id();
        let mut luna = Astro::luna(id, codigo, proyecto, x, y);
        let t = aterrizaje(&self.astros, &luna, x, y, &[])?;
        luna.padre = t.padre;
        let propio = self.empezar();
        self.apuntar_astro(id);
        self.astros.push(luna);
        self.terminar(propio);
        Ok(id)
    }

    pub fn crear_planeta(&mut self, x: f32, y: f32, radio: f32) -> Result<IdAstro, Rechazo> {
        let id = self.nuevo_id();
        let mut p = Astro::planeta(id, x, y, radio);
        let t = aterrizaje(&self.astros, &p, x, y, &[])?;
        p.padre = t.padre;
        let propio = self.empezar();
        self.apuntar_astro(id);
        self.astros.push(p);
        self.terminar(propio);
        Ok(id)
    }

    /// D230. Devuelve cuantos astros salieron del cielo. Las galaxias no.
    pub fn borrar(&mut self, ids: &[IdAstro], escena: &mut Escena) -> usize {
        let mut quitar: Vec<IdAstro> = Vec::new();
        let mut anot: Vec<u64> = Vec::new();
        for id in ids {
            let Some(a) = self.astro(*id) else { continue };
            match a.clase {
                Clase::Galaxia { .. } => {}
                Clase::Planeta => {
                    quitar.push(*id);
                    quitar.extend(descendientes(&self.astros, *id));
                    anot.extend(self.anotaciones_dentro(*id, escena));
                }
                Clase::Luna { .. } => quitar.push(*id),
            }
        }
        quitar.sort();
        quitar.dedup();
        anot.sort();
        anot.dedup();
        if quitar.is_empty() {
            return 0;
        }
        let propio = self.empezar();
        for id in &quitar {
            self.apuntar_astro(*id);
        }
        let lineas: Vec<u64> = self
            .conexiones
            .iter()
            .filter(|c| quitar.iter().any(|q| c.toca(*q)))
            .map(|c| c.id)
            .collect();
        for c in &lineas {
            self.apuntar_conexion(*c);
        }
        self.astros.retain(|a| !quitar.contains(&a.id));
        self.conexiones.retain(|c| !lineas.contains(&c.id));
        if !anot.is_empty() {
            escena.abrir_paso();
            for e in &anot {
                escena.borrar_apuntando(*e);
            }
            escena.cerrar_paso();
            self.marcar_anotaciones(escena);
        }
        self.terminar(propio);
        quitar.len()
    }

    /// D233.
    pub fn agrupar_en_planeta(&mut self, lunas: &[IdAstro]) -> Result<IdAstro, Rechazo> {
        let ls: Vec<Astro> = lunas
            .iter()
            .filter_map(|id| self.astro(*id).cloned())
            .collect();
        if ls.is_empty() {
            return Err(Rechazo::NoExiste);
        }
        let n = ls.len() as f32;
        let (cx, cy) = (
            ls.iter().map(|l| l.x).sum::<f32>() / n,
            ls.iter().map(|l| l.y).sum::<f32>() / n,
        );
        let lejos = ls
            .iter()
            .map(|l| {
                ((l.x - cx).powi(2) + (l.y - cy).powi(2)).sqrt()
                    + RADIO_LUNA * std::f32::consts::SQRT_2
            })
            .fold(0.0, f32::max);
        let radio = (lejos * 1.2).max(RADIO_PLANETA_S);
        let propio = self.empezar();
        let hecho = (|| {
            let p = self.crear_planeta(cx, cy, radio)?;
            for l in lunas {
                let luna = self.astro(*l).cloned().ok_or(Rechazo::NoExiste)?;
                let t = aterrizaje(&self.astros, &luna, luna.x, luna.y, &[*l])?;
                self.apuntar_astro(*l);
                if let Some(x) = self.astros.iter_mut().find(|x| x.id == *l) {
                    x.padre = t.padre;
                }
            }
            Ok(p)
        })();
        match hecho {
            Ok(p) => {
                self.terminar(propio);
                Ok(p)
            }
            Err(e) => {
                self.cancelar(propio);
                Err(e)
            }
        }
    }

    /// D232: planetas en arco por la mitad de arriba; lunas en la orbita de
    /// cada planeta. Solo cuando se pide.
    pub fn ordenar_galaxia(&mut self, g: IdAstro) {
        let Some(gal) = self.astro(g).cloned() else {
            return;
        };
        let planetas: Vec<IdAstro> = self
            .hijos(g)
            .filter(|a| matches!(a.clase, Clase::Planeta))
            .map(|a| a.id)
            .collect();
        let propio = self.empezar();
        let n = planetas.len() as f32;
        for (i, p) in planetas.iter().enumerate() {
            // Angulos entre pi y 2 pi: la mitad de arriba en pantalla (y baja).
            let ang = std::f32::consts::PI * (1.0 + (i as f32 + 1.0) / (n + 1.0));
            let (px, py) = (
                gal.x + gal.radio * 0.5 * ang.cos(),
                gal.y + gal.radio * 0.5 * ang.sin(),
            );
            self.apuntar_astro(*p);
            let radio = {
                let x = self
                    .astros
                    .iter_mut()
                    .find(|x| x.id == *p)
                    .expect("es hijo");
                x.x = px;
                x.y = py;
                x.radio
            };
            let lunas: Vec<IdAstro> = self.hijos(*p).map(|a| a.id).collect();
            let m = lunas.len().max(1) as f32;
            for (j, l) in lunas.iter().enumerate() {
                let a2 = std::f32::consts::TAU * j as f32 / m;
                let orbita = (radio - RADIO_LUNA * std::f32::consts::SQRT_2).max(0.0) * 0.8;
                self.apuntar_astro(*l);
                if let Some(x) = self.astros.iter_mut().find(|x| x.id == *l) {
                    x.x = px + orbita * a2.cos();
                    x.y = py + orbita * a2.sin();
                }
            }
        }
        self.terminar(propio);
    }

    pub fn conectar(&mut self, desde: IdAstro, hasta: IdAstro, tipo: TipoConexion) -> Option<u64> {
        if desde == hasta || self.astro(desde).is_none() || self.astro(hasta).is_none() {
            return None;
        }
        let repetida = self.conexiones.iter().any(|c| {
            c.tipo == tipo
                && ((c.desde == desde && c.hasta == hasta)
                    || (c.desde == hasta && c.hasta == desde))
        });
        if repetida {
            return None;
        }
        let id = self.nuevo_id().0;
        let propio = self.empezar();
        self.apuntar_conexion(id);
        let mut c = Conexion::nueva(id, desde, hasta);
        c.tipo = tipo;
        self.conexiones.push(c);
        self.terminar(propio);
        Some(id)
    }

    pub fn borrar_conexion(&mut self, id: u64) {
        let propio = self.empezar();
        self.apuntar_conexion(id);
        self.conexiones.retain(|c| c.id != id);
        self.terminar(propio);
    }

    pub fn editar_conexion(&mut self, id: u64, f: impl FnOnce(&mut Conexion)) {
        let propio = self.empezar();
        self.apuntar_conexion(id);
        if let Some(c) = self.conexiones.iter_mut().find(|c| c.id == id) {
            f(c);
        }
        self.terminar(propio);
    }

    /// Nombre, emoji, color, nota, radio, notas del chat. No mueve: para
    /// mover esta el arrastre, que comprueba donde cae.
    pub fn editar(&mut self, id: IdAstro, f: impl FnOnce(&mut Astro)) {
        let propio = self.empezar();
        self.apuntar_astro(id);
        if let Some(a) = self.astros.iter_mut().find(|a| a.id == id) {
            let (x, y, padre, clase) = (a.x, a.y, a.padre, a.clase.clone());
            f(a);
            // Lo que no se edita aqui, se deja como estaba.
            a.x = x;
            a.y = y;
            a.padre = padre;
            a.clase = clase;
        }
        self.terminar(propio);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::astro::{Astro, IdAstro, TipoConexion};
    use pixpin_motor2d::{ColorRgba, Elemento, Escena, EstiloTrazo, Figura};

    // `Elemento` no deriva `Default`: se escribe entero, como en las pruebas
    // de `indice.rs` del motor.
    fn rect(x: f32, y: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho: 20.0,
            alto: 20.0,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: None,
            grosor: 2.0,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
        }
    }

    /// g1 (p1) en 0,0 con un planeta en 500,0; g2 (p2) en 6000,0.
    fn cielo() -> (Universo, IdAstro, IdAstro, IdAstro) {
        let mut u = Universo::nuevo();
        let g1 = u.nuevo_id();
        let g2 = u.nuevo_id();
        u.astros.push(Astro::galaxia(g1, "p1", 0.0, 0.0));
        u.astros.push(Astro::galaxia(g2, "p2", 6000.0, 0.0));
        let pl = u.crear_planeta(500.0, 0.0, 400.0).unwrap();
        (u, g1, g2, pl)
    }

    #[test]
    fn mover_un_planeta_mueve_sus_lunas_y_las_anotaciones_enteras_dentro_y_nada_mas() {
        let (mut u, _, _, pl) = cielo();
        let luna = u.colocar_luna("m:a", "p1", 500.0, 0.0).unwrap();
        let mut e = Escena::nueva();
        let dentro = e.anadir(rect(490.0, 90.0));
        let fuera = e.anadir(rect(-1500.0, 0.0));
        let mut a = u.empezar_arrastre(&[pl], &e);
        u.arrastrar(&mut a, 100.0, 50.0, &mut e);
        u.soltar(a, &mut e).unwrap();
        assert_eq!(
            (u.astro(luna).unwrap().x, u.astro(luna).unwrap().y),
            (600.0, 50.0)
        );
        assert_eq!(e.buscar(dentro).unwrap().x, 590.0);
        assert_eq!(e.buscar(fuera).unwrap().x, -1500.0);
    }

    #[test]
    fn un_arrastre_rechazado_devuelve_todo_a_su_sitio_y_no_deja_paso() {
        let (mut u, _, _, _) = cielo();
        let luna = u.colocar_luna("m:a", "p1", -1000.0, 0.0).unwrap();
        let mut e = Escena::nueva();
        let pasos = e.pasos_cerrados();
        let mut a = u.empezar_arrastre(&[luna], &e);
        u.arrastrar(&mut a, 7000.0, 0.0, &mut e); // a la galaxia de p2
        assert_eq!(u.soltar(a, &mut e), Err(Rechazo::OtraGalaxia));
        assert_eq!(u.astro(luna).unwrap().x, -1000.0);
        assert_eq!(e.pasos_cerrados(), pasos);
        assert!(
            u.deshacer(&mut e),
            "queda el paso de colocar, no el del arrastre"
        );
        assert!(u.astro(luna).is_none());
    }

    #[test]
    fn soltar_un_planeta_fuera_lo_hace_exoplaneta_y_deshacer_lo_devuelve() {
        let (mut u, g1, _, pl) = cielo();
        let e0 = Escena::nueva();
        let mut e = e0.clone();
        let mut a = u.empezar_arrastre(&[pl], &e);
        u.arrastrar(&mut a, 20_000.0, 0.0, &mut e);
        u.soltar(a, &mut e).unwrap();
        assert_eq!(u.astro(pl).unwrap().padre, None);
        assert!(u.deshacer(&mut e));
        assert_eq!(u.astro(pl).unwrap().padre, Some(g1));
        assert_eq!(u.astro(pl).unwrap().x, 500.0);
        assert!(u.rehacer(&mut e));
        assert_eq!(u.astro(pl).unwrap().x, 20_500.0);
    }

    #[test]
    fn una_luna_no_se_coloca_dos_veces() {
        let (mut u, _, _, _) = cielo();
        u.colocar_luna("m:a", "p1", 0.0, 0.0).unwrap();
        assert_eq!(
            u.colocar_luna("m:a", "p1", 10.0, 0.0),
            Err(Rechazo::YaColocada)
        );
    }

    #[test]
    fn borrar_un_planeta_devuelve_sus_lunas_a_la_nebulosa_y_borra_sus_anotaciones() {
        let (mut u, g1, _, pl) = cielo();
        let luna = u.colocar_luna("m:a", "p1", 500.0, 0.0).unwrap();
        let mut e = Escena::nueva();
        let dentro = e.anadir(rect(490.0, 90.0));
        u.conectar(pl, g1, TipoConexion::Depende).unwrap();
        assert_eq!(u.borrar(&[pl], &mut e), 2);
        assert!(
            u.astro(luna).is_none(),
            "la luna vuelve a la nebulosa: sin entrada"
        );
        assert!(e.buscar(dentro).is_none_or(|x| x.borrado));
        assert!(u.conexiones.is_empty());
        assert!(u.deshacer(&mut e));
        assert!(u.astro(luna).is_some());
        assert!(!e.buscar(dentro).unwrap().borrado);
        assert_eq!(u.conexiones.len(), 1);
    }

    #[test]
    fn una_galaxia_no_se_borra() {
        let (mut u, g1, _, _) = cielo();
        let mut e = Escena::nueva();
        assert_eq!(u.borrar(&[g1], &mut e), 0);
        assert!(u.astro(g1).is_some());
    }

    #[test]
    fn agrupar_crea_un_planeta_que_contiene_a_todas_con_margen() {
        let (mut u, g1, _, _) = cielo();
        let a = u.colocar_luna("m:a", "p1", -1000.0, -200.0).unwrap();
        let b = u.colocar_luna("m:b", "p1", -600.0, 200.0).unwrap();
        let p = u.agrupar_en_planeta(&[a, b]).unwrap();
        let pl = u.astro(p).unwrap().clone();
        assert_eq!(pl.padre, Some(g1));
        for l in [a, b] {
            let luna = u.astro(l).unwrap();
            assert_eq!(luna.padre, Some(p));
            assert!(pl.contiene_caja(luna.caja()));
        }
    }

    #[test]
    fn conectar_dos_veces_lo_mismo_o_consigo_mismo_no_hace_nada() {
        let (mut u, g1, g2, _) = cielo();
        assert!(u.conectar(g1, g2, TipoConexion::Relacion).is_some());
        assert!(u.conectar(g1, g2, TipoConexion::Relacion).is_none());
        assert!(u.conectar(g1, g1, TipoConexion::Relacion).is_none());
        assert!(
            u.conectar(g1, IdAstro(999), TipoConexion::Relacion)
                .is_none()
        );
    }

    #[test]
    fn ordenar_pone_los_planetas_en_la_mitad_de_arriba_y_sus_lunas_dentro() {
        let (mut u, g1, _, pl) = cielo();
        u.crear_planeta(-500.0, 0.0, 250.0).unwrap();
        let l = u.colocar_luna("m:a", "p1", 500.0, 0.0).unwrap();
        u.ordenar_galaxia(g1);
        for p in u.hijos(g1).filter(|a| a.es_contenedor()) {
            assert!(p.y < 0.0, "arriba: {}", p.y);
        }
        let planeta = u.astro(pl).unwrap().clone();
        assert!(planeta.contiene_caja(u.astro(l).unwrap().caja()));
    }

    #[test]
    fn un_trazo_del_editor_entra_en_la_pila_y_se_deshace_en_orden() {
        let (mut u, _, _, pl) = cielo();
        let mut e = Escena::nueva();
        let id = e.anadir(rect(0.0, 0.0));
        e.abrir_paso();
        e.borrar_apuntando(id);
        e.cerrar_paso();
        u.sincronizar_trazos(&e);
        u.editar(pl, |a| a.nombre = "Planos".into());
        assert!(u.deshacer(&mut e)); // el nombre
        assert_eq!(u.astro(pl).unwrap().nombre, "");
        assert!(u.deshacer(&mut e)); // el borrado del trazo
        assert!(!e.buscar(id).unwrap().borrado);
    }

    #[test]
    fn la_pila_no_pasa_de_doscientos() {
        let (mut u, _, _, pl) = cielo();
        let mut e = Escena::nueva();
        for i in 0..250 {
            u.editar(pl, |a| a.nombre = i.to_string());
        }
        let mut n = 0;
        while u.deshacer(&mut e) {
            n += 1;
        }
        assert_eq!(n, crate::historia::TOPE);
    }
}
