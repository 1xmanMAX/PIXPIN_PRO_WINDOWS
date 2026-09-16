//! El estilo del panel lateral de Excalidraw: con que nace lo que se dibuja
//! y como se cambia lo que ya esta dibujado.
//!
//! En Excalidraw el panel hace las dos cosas a la vez: si hay algo elegido,
//! cambia eso; y SIEMPRE deja el valor como el de lo proximo que se dibuje
//! (`currentItem*` en su `appState`). Aqui igual: `EstiloDibujo` es ese
//! «actual» y `aplicar_a` lo lleva a la seleccion en un solo paso de
//! deshacer.

use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
use crate::escena::Escena;
use crate::seleccion::Seleccion;

/// Los tres grosores del panel de Excalidraw (`strokeWidth` 1, 2 y 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NivelGrosor {
    Fino,
    Medio,
    Grueso,
}

impl NivelGrosor {
    /// El grosor de una figura (rectangulo, linea...), en unidades del mundo.
    pub fn de_forma(self) -> f32 {
        match self {
            NivelGrosor::Fino => 1.0,
            NivelGrosor::Medio => 2.0,
            NivelGrosor::Grueso => 4.0,
        }
    }

    /// El `strokeWidth` del lapiz, que la tinta multiplica por su factor.
    pub fn de_tinta(self) -> f32 {
        match self {
            NivelGrosor::Fino => crate::tinta::GROSOR_FINO,
            NivelGrosor::Medio => crate::tinta::GROSOR_MEDIO,
            NivelGrosor::Grueso => crate::tinta::GROSOR_GRUESO,
        }
    }

    /// El nivel mas cercano a un grosor ya dibujado, para marcar en el panel
    /// el boton que corresponde a lo seleccionado.
    pub fn de_elemento(figura: &Figura, grosor: f32) -> NivelGrosor {
        let (fino, medio, grueso) = match figura {
            Figura::Lapiz { .. } => (
                crate::tinta::GROSOR_FINO,
                crate::tinta::GROSOR_MEDIO,
                crate::tinta::GROSOR_GRUESO,
            ),
            _ => (1.0, 2.0, 4.0),
        };
        let d = |v: f32| (grosor - v).abs();
        if d(fino) <= d(medio) && d(fino) <= d(grueso) {
            NivelGrosor::Fino
        } else if d(medio) <= d(grueso) {
            NivelGrosor::Medio
        } else {
            NivelGrosor::Grueso
        }
    }
}

/// El «actual» del panel: con esto nace cada elemento nuevo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstiloDibujo {
    pub trazo: ColorRgba,
    /// `None` = transparente, el fondo por defecto de Excalidraw.
    pub relleno: Option<ColorRgba>,
    pub grosor: NivelGrosor,
    pub estilo: EstiloTrazo,
    /// 0 arquitecto, 1 artista, 2 caricaturista.
    pub rugosidad: f32,
    /// De 0 a 1.
    pub opacidad: f32,
}

impl Default for EstiloDibujo {
    /// Los valores por defecto de Excalidraw: trazo `#1e1e1e`, sin fondo,
    /// grosor medio, trazo continuo, «artista» y opaco.
    fn default() -> Self {
        let gris = 0x1e as f32 / 255.0;
        Self {
            trazo: ColorRgba::opaco(gris, gris, gris),
            relleno: None,
            grosor: NivelGrosor::Medio,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
        }
    }
}

/// Un control del panel pulsado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CambioEstilo {
    Trazo(ColorRgba),
    Relleno(Option<ColorRgba>),
    Grosor(NivelGrosor),
    Estilo(EstiloTrazo),
    Rugosidad(f32),
    Opacidad(f32),
}

impl EstiloDibujo {
    pub fn aplicar(&mut self, cambio: CambioEstilo) {
        match cambio {
            CambioEstilo::Trazo(c) => self.trazo = c,
            CambioEstilo::Relleno(c) => self.relleno = c,
            CambioEstilo::Grosor(g) => self.grosor = g,
            CambioEstilo::Estilo(e) => self.estilo = e,
            CambioEstilo::Rugosidad(r) => self.rugosidad = r.clamp(0.0, 2.0),
            CambioEstilo::Opacidad(o) => self.opacidad = o.clamp(0.0, 1.0),
        }
    }
}

/// El grosor que tendria `figura` con el nivel `g`.
fn grosor_para(figura: &Figura, g: NivelGrosor) -> f32 {
    match figura {
        Figura::Lapiz { .. } => g.de_tinta(),
        _ => g.de_forma(),
    }
}

/// Lleva el cambio a todo lo seleccionado, como UN paso de deshacer. Solo
/// toca lo que cambia de verdad: pulsar el color que ya tiene no ensucia el
/// historial.
pub fn aplicar_a(escena: &mut Escena, sel: &Seleccion, cambio: CambioEstilo) {
    let cambia = |e: &crate::elemento::Elemento| match cambio {
        CambioEstilo::Trazo(c) => e.trazo != c,
        CambioEstilo::Relleno(c) => e.relleno != c,
        CambioEstilo::Grosor(g) => e.grosor != grosor_para(&e.figura, g),
        CambioEstilo::Estilo(s) => e.estilo != s,
        CambioEstilo::Rugosidad(r) => e.rugosidad != r.clamp(0.0, 2.0),
        CambioEstilo::Opacidad(o) => e.opacidad != o.clamp(0.0, 1.0),
    };
    let afectados: Vec<u64> = sel
        .ids()
        .iter()
        .copied()
        .filter(|id| escena.buscar(*id).is_some_and(|e| !e.borrado && cambia(e)))
        .collect();
    if afectados.is_empty() {
        return;
    }
    escena.abrir_paso();
    for id in afectados {
        escena.apuntar_edicion(id);
        if let Some(e) = escena.buscar_mut(id) {
            match cambio {
                CambioEstilo::Trazo(c) => e.trazo = c,
                CambioEstilo::Relleno(c) => e.relleno = c,
                CambioEstilo::Grosor(g) => e.grosor = grosor_para(&e.figura, g),
                CambioEstilo::Estilo(s) => e.estilo = s,
                CambioEstilo::Rugosidad(r) => e.rugosidad = r.clamp(0.0, 2.0),
                CambioEstilo::Opacidad(o) => e.opacidad = o.clamp(0.0, 1.0),
            }
            // Sin subir la version la cache seguiria pintando el aspecto
            // viejo.
            e.tocar();
        }
    }
    escena.cerrar_paso();
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::Elemento;

    fn rect(escena: &mut Escena) -> u64 {
        escena.anadir(Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x: 0.0,
            y: 0.0,
            ancho: 10.0,
            alto: 10.0,
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
        })
    }

    #[test]
    fn cambiar_el_color_de_lo_elegido_es_un_solo_paso_que_se_deshace() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let b = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        aplicar_a(&mut escena, &sel, CambioEstilo::Trazo(rojo));
        assert_eq!(escena.buscar(a).unwrap().trazo, rojo);
        assert_eq!(escena.buscar(b).unwrap().trazo, rojo);
        assert!(escena.deshacer());
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        assert_eq!(escena.buscar(a).unwrap().trazo, negro);
        assert_eq!(escena.buscar(b).unwrap().trazo, negro);
    }

    #[test]
    fn el_cambio_sube_la_version_para_que_la_cache_repinte() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let antes = escena.buscar(a).unwrap().version;
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        aplicar_a(&mut escena, &sel, CambioEstilo::Opacidad(0.5));
        assert!(escena.buscar(a).unwrap().version > antes);
    }

    #[test]
    fn pulsar_el_valor_que_ya_tiene_no_cambia_nada() {
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let antes = escena.buscar(a).unwrap().clone();
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        // El rectangulo ya tenia 2 (medio).
        aplicar_a(&mut escena, &sel, CambioEstilo::Grosor(NivelGrosor::Medio));
        assert_eq!(escena.buscar(a).unwrap(), &antes, "ni version subida");
    }

    #[test]
    fn el_grosor_del_lapiz_va_en_su_escala_y_el_de_la_forma_en_la_suya() {
        assert_eq!(NivelGrosor::Grueso.de_forma(), 4.0);
        assert_eq!(NivelGrosor::Grueso.de_tinta(), crate::tinta::GROSOR_GRUESO);
        let lapiz = Figura::Lapiz {
            puntos: vec![],
            presiones: vec![],
            opciones: None,
        };
        assert_eq!(
            NivelGrosor::de_elemento(&lapiz, crate::tinta::GROSOR_FINO),
            NivelGrosor::Fino
        );
        assert_eq!(
            NivelGrosor::de_elemento(&Figura::Rectangulo, 3.9),
            NivelGrosor::Grueso
        );
    }

    #[test]
    fn el_estilo_por_defecto_es_el_de_excalidraw_y_los_limites_se_respetan() {
        let mut e = EstiloDibujo::default();
        assert_eq!(e.relleno, None);
        assert_eq!(e.grosor, NivelGrosor::Medio);
        e.aplicar(CambioEstilo::Opacidad(3.0));
        assert_eq!(e.opacidad, 1.0);
        e.aplicar(CambioEstilo::Rugosidad(-1.0));
        assert_eq!(e.rugosidad, 0.0);
    }
}
