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
use crate::relleno::EstiloRelleno;
use crate::seleccion::Seleccion;
use crate::tinta::MaterialTinta;

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
    /// Como se pinta ese fondo: solido, rayado o cruzado.
    pub estilo_relleno: EstiloRelleno,
    pub grosor: NivelGrosor,
    pub estilo: EstiloTrazo,
    /// 0 arquitecto, 1 artista, 2 caricaturista.
    pub rugosidad: f32,
    /// De 0 a 1.
    pub opacidad: f32,
    /// **De que esta hecha la tinta**: lisa, con grano o encendida. Va con el
    /// color y no con la herramienta, porque es la otra mitad de «de que
    /// color va esto» (misma razon que en el movil).
    pub material: MaterialTinta,
}

impl Default for EstiloDibujo {
    /// Los valores por defecto de Excalidraw: trazo `#1e1e1e`, sin fondo,
    /// relleno rayado, grosor medio, trazo continuo, «artista» y opaco.
    fn default() -> Self {
        let gris = 0x1e as f32 / 255.0;
        Self {
            trazo: ColorRgba::opaco(gris, gris, gris),
            relleno: None,
            estilo_relleno: EstiloRelleno::default(),
            grosor: NivelGrosor::Medio,
            estilo: EstiloTrazo::Solido,
            rugosidad: 1.0,
            opacidad: 1.0,
            material: MaterialTinta::Lisa,
        }
    }
}

/// Un control del panel pulsado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CambioEstilo {
    Trazo(ColorRgba),
    Relleno(Option<ColorRgba>),
    /// Como se pinta el fondo: solido, rayado o cruzado.
    EstiloRelleno(EstiloRelleno),
    Grosor(NivelGrosor),
    Estilo(EstiloTrazo),
    Rugosidad(f32),
    Opacidad(f32),
    /// De que esta hecha la tinta. Ver `tinta::material`.
    Material(MaterialTinta),
}

impl EstiloDibujo {
    pub fn aplicar(&mut self, cambio: CambioEstilo) {
        match cambio {
            CambioEstilo::Trazo(c) => self.trazo = c,
            CambioEstilo::Relleno(c) => self.relleno = c,
            CambioEstilo::EstiloRelleno(r) => self.estilo_relleno = r,
            CambioEstilo::Grosor(g) => self.grosor = g,
            CambioEstilo::Estilo(e) => self.estilo = e,
            CambioEstilo::Rugosidad(r) => self.rugosidad = r.clamp(0.0, 2.0),
            CambioEstilo::Opacidad(o) => self.opacidad = o.clamp(0.0, 1.0),
            CambioEstilo::Material(m) => self.material = m,
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
        CambioEstilo::EstiloRelleno(r) => e.estilo_relleno != r,
        CambioEstilo::Grosor(g) => e.grosor != grosor_para(&e.figura, g),
        CambioEstilo::Estilo(s) => e.estilo != s,
        CambioEstilo::Rugosidad(r) => e.rugosidad != r.clamp(0.0, 2.0),
        CambioEstilo::Opacidad(o) => e.opacidad != o.clamp(0.0, 1.0),
        CambioEstilo::Material(m) => e.material != m,
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
                CambioEstilo::EstiloRelleno(r) => e.estilo_relleno = r,
                CambioEstilo::Grosor(g) => e.grosor = grosor_para(&e.figura, g),
                CambioEstilo::Estilo(s) => e.estilo = s,
                CambioEstilo::Rugosidad(r) => e.rugosidad = r.clamp(0.0, 2.0),
                CambioEstilo::Opacidad(o) => e.opacidad = o.clamp(0.0, 1.0),
                CambioEstilo::Material(m) => e.material = m,
            }
            // Sin subir la version la cache seguiria pintando el aspecto
            // viejo.
            e.tocar();
        }
    }
    escena.cerrar_paso();
}

// -------------------------------------------------------------------------
// Copiar estilo
// -------------------------------------------------------------------------

/// Que se le puede tocar a un tipo de figura (`Propiedad` del movil,
/// `DrawProperties.kt:18-88`).
///
/// Son **las del PC**: del enumerado del movil faltan aqui las que hablan de
/// campos que este motor todavia no tiene (`VOLUMEN`, `LUPA`, `OSCURECER`,
/// `ZONA`, `PRESION`, `ESTILO_DE_TEXTO`, `FORMA_FLECHA`). Cuando su grupo las
/// traiga, se anaden aqui y a la tabla de abajo a la vez, que es el punto de
/// que la tabla exista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propiedad {
    Trazo,
    Fondo,
    /// Como se pinta el fondo: solido, rayado, cruzado.
    Relleno,
    /// Continuo, a trazos o de puntos.
    Linea,
    Grosor,
    Rugosidad,
    /// Esquinas redondeadas.
    Esquinas,
    /// De que esta hecha la tinta.
    Material,
    Opacidad,
}

/// **La tabla**: que mandos aplican a que tipo (`propiedadesDeTipo`,
/// `DrawProperties.kt:169-303`).
///
/// Es la misma tabla para dos cosas, y de eso depende todo: decide **que
/// controles salen** en el panel y decide **que se escribe encima** al pegar
/// un estilo. Con dos tablas, un panel acaba ofreciendo algo que no se
/// guarda, o guardando algo que no se ofrece.
pub fn propiedades_de(figura: &Figura) -> &'static [Propiedad] {
    use Propiedad::*;
    match figura {
        // Las dos que se rellenan y se redondean.
        Figura::Rectangulo | Figura::Rombo => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Esquinas, Opacidad,
        ],
        // La elipse no tiene esquinas que redondear.
        Figura::Elipse => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Opacidad,
        ],
        // La linea si admite fondo: cerrada sobre si misma se rellena.
        Figura::Linea { .. } => &[
            Trazo, Fondo, Relleno, Linea, Grosor, Material, Rugosidad, Esquinas, Opacidad,
        ],
        // La flecha no encierra nada, asi que ni fondo ni esquinas.
        Figura::Flecha { .. } => &[Trazo, Linea, Grosor, Material, Rugosidad, Opacidad],
        // El lapiz no lleva linea discontinua ni esquinas, que serian botones
        // muertos. **Fondo y relleno si**, y hacen falta justamente para
        // poder apagarlos: un garabato que se cierra sobre si mismo se rellena
        // solo. Se quitaron una vez por creer que sobraban y lo que se
        // consiguio fue dejar la trama puesta y sin interruptor.
        Figura::Lapiz { .. } | Figura::Resaltador { .. } => {
            &[Trazo, Fondo, Relleno, Grosor, Material, Rugosidad, Opacidad]
        }
        // El arco es una raya curva: ni fondo ni relleno.
        Figura::Arco { .. } => &[Trazo, Linea, Grosor, Material, Rugosidad, Opacidad],
        // El texto y el circulito numerado: color y nada mas de lo que aqui
        // hay. El tamano de letra no viaja en `EstiloDibujo`, asi que no
        // entra en la copia.
        Figura::Texto { .. } | Figura::Serie { .. } | Figura::Punto { .. } => &[Trazo, Opacidad],
        // La cota se pinta lisa a proposito, para que se vea exactamente
        // donde acaba la medida: ni fondo, ni relleno, ni rugosidad.
        Figura::Cota { .. } => &[Trazo, Grosor, Material, Opacidad],
        // La reglita: ni relleno ni rugosidad, que una escala temblorosa no
        // se lee.
        Figura::EscalaGrafica => &[Trazo, Opacidad],
        // El relleno de un hueco es **solo mancha**: su borde lo dibujan las
        // figuras que lo encierran, asi que ofrecerle color de trazo seria un
        // boton que no puede cambiar nada de lo que se ve.
        Figura::Region { .. } => &[Fondo, Relleno, Opacidad],
        // En el mosaico el grosor hace de tamano de grano.
        Figura::Mosaico { .. } => &[Grosor, Material, Opacidad],
        // La imagen y el emoji: solo lo que los tapa o los redondea.
        Figura::Imagen { .. } => &[Esquinas, Opacidad],
        Figura::Emoji { .. } => &[Opacidad],
        // El foco no dibuja tinta: apaga lo de alrededor. Lo que gradua es
        // cuanto oscurece, y eso aqui todavia no es un mando.
        Figura::Foco { .. } => &[],
        // La hoja no tiene estilo: es un limite, no un dibujo. Solo se
        // estira.
        Figura::Marco { .. } => &[],
    }
}

/// El estilo tomado de una figura, listo para pegarlo en otras.
///
/// Guarda **todos** los valores aunque el tipo de origen no admita todos:
/// quien decide que se escribe es el cruce de las dos tablas al pegar, y
/// recortar ya aqui obligaria a mirar el tipo de origen dos veces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstiloCopiado {
    /// Que mandos tenia el tipo del que se copio.
    origen: &'static [Propiedad],
    trazo: ColorRgba,
    relleno: Option<ColorRgba>,
    estilo_relleno: EstiloRelleno,
    grosor: NivelGrosor,
    estilo: EstiloTrazo,
    rugosidad: f32,
    opacidad: f32,
    material: MaterialTinta,
    redondo: bool,
}

impl EstiloCopiado {
    /// Las propiedades que de verdad se llevo.
    pub fn propiedades(&self) -> &'static [Propiedad] {
        self.origen
    }
}

/// **Copia el estilo de un elemento** (el cuentagotas de estilo).
///
/// # Por que existe si el movil no lo tiene
///
/// Porque es invencion de escritorio y ahi se gana. Con un raton y un teclado,
/// «ponle a esta caja lo mismo que a aquella» son dos clics; a dedo son seis
/// mandos del panel, uno por uno, mirando cual estaba puesto. Se busco en
/// todo el motor de Android —incluida su paleta de colores, 431 lineas— y no
/// existe.
///
/// Lo que si es del movil es **la tabla**: [`propiedades_de`] dice
/// exactamente que campos tiene sentido llevarse de cada tipo, y es la misma
/// que decide que mandos ensena el panel.
pub fn copiar(e: &crate::elemento::Elemento) -> EstiloCopiado {
    EstiloCopiado {
        origen: propiedades_de(&e.figura),
        trazo: e.trazo,
        relleno: e.relleno,
        estilo_relleno: e.estilo_relleno,
        grosor: NivelGrosor::de_elemento(&e.figura, e.grosor),
        estilo: e.estilo,
        rugosidad: e.rugosidad,
        opacidad: e.opacidad,
        material: e.material,
        redondo: e.redondo,
    }
}

/// **Pega el estilo copiado sobre la seleccion**, en un solo paso de
/// deshacer.
///
/// Solo se escribe lo que admiten **los dos** tipos, el de origen y el de
/// destino. Es la regla que hace que copiar de un rectangulo a un texto
/// lleve el color y no el relleno rayado —que en un rotulo no significa
/// nada—, y que copiar de un texto a un rectangulo no le borre el fondo que
/// ya tenia.
///
/// Devuelve cuantos cambiaron. Como el resto del panel, no ensucia el
/// historial si no hay nada que cambiar.
pub fn pegar_a(escena: &mut Escena, sel: &Seleccion, copiado: &EstiloCopiado) -> usize {
    let afectados: Vec<u64> = sel
        .ids()
        .iter()
        .copied()
        .filter(|id| {
            escena
                .buscar(*id)
                .is_some_and(|e| !e.borrado && !e.bloqueado && cambiaria(e, copiado))
        })
        .collect();
    if afectados.is_empty() {
        return 0;
    }
    escena.abrir_paso();
    let cuantos = afectados.len();
    for id in afectados {
        escena.apuntar_edicion(id);
        let Some(e) = escena.buscar_mut(id) else {
            continue;
        };
        for p in comunes(e, copiado) {
            aplicar_propiedad(e, p, copiado);
        }
        // Sin subir la version la cache seguiria pintando el aspecto viejo.
        e.tocar();
    }
    escena.cerrar_paso();
    cuantos
}

/// Las propiedades que admiten a la vez el origen y el destino.
fn comunes(e: &crate::elemento::Elemento, c: &EstiloCopiado) -> Vec<Propiedad> {
    propiedades_de(&e.figura)
        .iter()
        .copied()
        .filter(|p| c.origen.contains(p))
        .collect()
}

fn aplicar_propiedad(e: &mut crate::elemento::Elemento, p: Propiedad, c: &EstiloCopiado) {
    match p {
        Propiedad::Trazo => e.trazo = c.trazo,
        Propiedad::Fondo => e.relleno = c.relleno,
        Propiedad::Relleno => e.estilo_relleno = c.estilo_relleno,
        Propiedad::Linea => e.estilo = c.estilo,
        // El grosor viaja como NIVEL y no como numero: el lapiz y las
        // figuras miden en escalas distintas, asi que copiar el 2 de un
        // rectangulo a un lapiz le pondria un trazo que no es ninguno de sus
        // tres.
        Propiedad::Grosor => e.grosor = grosor_para(&e.figura, c.grosor),
        Propiedad::Rugosidad => e.rugosidad = c.rugosidad,
        Propiedad::Esquinas => e.redondo = c.redondo,
        Propiedad::Material => e.material = c.material,
        Propiedad::Opacidad => e.opacidad = c.opacidad,
    }
}

fn cambiaria(e: &crate::elemento::Elemento, c: &EstiloCopiado) -> bool {
    comunes(e, c).into_iter().any(|p| match p {
        Propiedad::Trazo => e.trazo != c.trazo,
        Propiedad::Fondo => e.relleno != c.relleno,
        Propiedad::Relleno => e.estilo_relleno != c.estilo_relleno,
        Propiedad::Linea => e.estilo != c.estilo,
        Propiedad::Grosor => e.grosor != grosor_para(&e.figura, c.grosor),
        Propiedad::Rugosidad => e.rugosidad != c.rugosidad,
        Propiedad::Esquinas => e.redondo != c.redondo,
        Propiedad::Material => e.material != c.material,
        Propiedad::Opacidad => e.opacidad != c.opacidad,
    })
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
            bloqueado: false,
            enlace: None,
            redondo: false,
            material: Default::default(),
            extras: Default::default(),
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
    fn el_relleno_elegido_va_a_lo_seleccionado_y_queda_para_lo_proximo() {
        // Las dos mitades del panel de Excalidraw: cambia lo elegido y ademas
        // deja el valor como el de lo proximo que se dibuje.
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner(a);
        let cambio = CambioEstilo::EstiloRelleno(EstiloRelleno::Cruzado);
        aplicar_a(&mut escena, &sel, cambio);
        assert_eq!(
            escena.buscar(a).unwrap().estilo_relleno,
            EstiloRelleno::Cruzado
        );

        let mut actual = EstiloDibujo::default();
        actual.aplicar(cambio);
        assert_eq!(actual.estilo_relleno, EstiloRelleno::Cruzado);

        // Y se deshace en un solo paso, como los demas cambios de estilo.
        assert!(escena.deshacer());
        assert_eq!(
            escena.buscar(a).unwrap().estilo_relleno,
            EstiloRelleno::default()
        );
    }

    #[test]
    fn cambiar_el_relleno_de_nada_no_abre_un_paso_de_deshacer_vacio() {
        // Caso negativo: sin seleccion no hay a quien aplicarselo, y un paso
        // vacio en el historial obligaria a pulsar deshacer dos veces para
        // deshacer una sola cosa.
        let mut escena = Escena::nueva();
        let a = rect(&mut escena);
        let vacia = Seleccion::nueva();
        aplicar_a(
            &mut escena,
            &vacia,
            CambioEstilo::EstiloRelleno(EstiloRelleno::Solido),
        );
        // El unico paso del historial sigue siendo el de haber anadido el
        // rectangulo: deshacer una vez tiene que quitarlo.
        assert!(escena.deshacer());
        assert!(escena.buscar(a).is_none_or(|e| e.borrado));
    }

    #[test]
    fn el_estilo_por_defecto_es_el_de_excalidraw_y_los_limites_se_respetan() {
        let mut e = EstiloDibujo::default();
        assert_eq!(e.relleno, None);
        // El rayado, no el solido: es lo que dibuja Excalidraw sin `fillStyle`.
        assert_eq!(e.estilo_relleno, EstiloRelleno::Rayado);
        assert_eq!(e.grosor, NivelGrosor::Medio);
        e.aplicar(CambioEstilo::Opacidad(3.0));
        assert_eq!(e.opacidad, 1.0);
        e.aplicar(CambioEstilo::Rugosidad(-1.0));
        assert_eq!(e.rugosidad, 0.0);
    }

    // --- Copiar estilo ---

    fn con(escena: &mut Escena, figura: Figura) -> u64 {
        let id = rect(escena);
        escena.buscar_mut(id).unwrap().figura = figura;
        id
    }

    fn sel_de(id: u64) -> Seleccion {
        let mut s = Seleccion::nueva();
        s.poner(id);
        s
    }

    #[test]
    fn copiar_un_rectangulo_en_otro_lleva_todo_su_estilo() {
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let destino = rect(&mut escena);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        {
            let e = escena.buscar_mut(origen).unwrap();
            e.trazo = rojo;
            e.relleno = Some(rojo);
            e.estilo = EstiloTrazo::Discontinuo;
            e.redondo = true;
            e.opacidad = 0.5;
        }
        let copiado = copiar(escena.buscar(origen).unwrap());
        assert_eq!(pegar_a(&mut escena, &sel_de(destino), &copiado), 1);

        let d = escena.buscar(destino).unwrap();
        assert_eq!(d.trazo, rojo);
        assert_eq!(d.relleno, Some(rojo));
        assert_eq!(d.estilo, EstiloTrazo::Discontinuo);
        assert!(d.redondo);
        assert_eq!(d.opacidad, 0.5);
    }

    #[test]
    fn de_un_rectangulo_a_un_texto_va_el_color_pero_no_el_relleno() {
        // La regla de la tabla: solo se escribe lo que admiten los dos. Un
        // rotulo con «relleno rayado» guardado no significa nada, y un fondo
        // puesto ahi seria un dato invisible que luego viaja al movil.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let verde = ColorRgba::opaco(0.1, 0.6, 0.2);
        {
            let e = escena.buscar_mut(origen).unwrap();
            e.trazo = verde;
            e.relleno = Some(verde);
            e.estilo_relleno = EstiloRelleno::Cruzado;
        }
        let copiado = copiar(escena.buscar(origen).unwrap());

        let destino = con(
            &mut escena,
            Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
        );
        pegar_a(&mut escena, &sel_de(destino), &copiado);
        let d = escena.buscar(destino).unwrap();
        assert_eq!(d.trazo, verde, "el color si");
        assert_eq!(d.relleno, None, "el fondo no");
        assert_eq!(d.estilo_relleno, EstiloRelleno::Rayado, "la trama tampoco");
    }

    #[test]
    fn de_un_texto_a_un_rectangulo_no_se_le_borra_el_fondo_que_tenia() {
        // El otro lado de la misma regla, y el que mas molesta cuando falta:
        // copiar de algo que no tiene fondo no puede significar «quitale el
        // fondo».
        let mut escena = Escena::nueva();
        let origen = con(
            &mut escena,
            Figura::Texto {
                texto: "hola".into(),
                tam: 20.0,
                familia: "Segoe UI".into(),
            },
        );
        let copiado = copiar(escena.buscar(origen).unwrap());

        let destino = rect(&mut escena);
        let azul = ColorRgba::opaco(0.2, 0.3, 0.9);
        escena.buscar_mut(destino).unwrap().relleno = Some(azul);
        pegar_a(&mut escena, &sel_de(destino), &copiado);
        assert_eq!(escena.buscar(destino).unwrap().relleno, Some(azul));
    }

    #[test]
    fn el_grosor_viaja_como_nivel_y_no_como_numero() {
        // Copiar el 4 de un rectangulo a un lapiz le pondria un trazo que no
        // es ninguno de sus tres, porque miden en escalas distintas.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        escena.buscar_mut(origen).unwrap().grosor = NivelGrosor::Grueso.de_forma();
        let copiado = copiar(escena.buscar(origen).unwrap());

        let lapiz = con(
            &mut escena,
            Figura::Lapiz {
                puntos: vec![],
                presiones: vec![],
                opciones: None,
            },
        );
        pegar_a(&mut escena, &sel_de(lapiz), &copiado);
        assert_eq!(
            escena.buscar(lapiz).unwrap().grosor,
            crate::tinta::GROSOR_GRUESO
        );
    }

    #[test]
    fn pegar_es_un_solo_paso_de_deshacer_para_toda_la_seleccion() {
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let rojo = ColorRgba::opaco(0.88, 0.19, 0.19);
        escena.buscar_mut(origen).unwrap().trazo = rojo;
        let copiado = copiar(escena.buscar(origen).unwrap());
        let a = rect(&mut escena);
        let b = rect(&mut escena);
        let mut sel = Seleccion::nueva();
        sel.poner_todos([a, b]);
        assert_eq!(pegar_a(&mut escena, &sel, &copiado), 2);
        assert!(escena.deshacer());
        let negro = ColorRgba::opaco(0.0, 0.0, 0.0);
        assert_eq!(escena.buscar(a).unwrap().trazo, negro);
        assert_eq!(escena.buscar(b).unwrap().trazo, negro);
    }

    #[test]
    fn pegar_lo_mismo_que_ya_habia_no_ensucia_el_historial() {
        // Caso negativo: un paso vacio obliga a pulsar deshacer dos veces
        // para deshacer una sola cosa.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        let copiado = copiar(escena.buscar(origen).unwrap());
        let destino = rect(&mut escena);
        assert_eq!(pegar_a(&mut escena, &sel_de(destino), &copiado), 0);
        let antes = escena.buscar(destino).unwrap().clone();
        assert_eq!(escena.buscar(destino).unwrap(), &antes, "ni version subida");
    }

    #[test]
    fn a_un_marco_no_se_le_pega_nada_y_lo_bloqueado_no_se_toca() {
        // Caso negativo doble: la hoja es un limite, no un dibujo, y
        // bloquear es pedir que no cambie.
        let mut escena = Escena::nueva();
        let origen = rect(&mut escena);
        escena.buscar_mut(origen).unwrap().trazo = ColorRgba::opaco(1.0, 0.0, 0.0);
        let copiado = copiar(escena.buscar(origen).unwrap());

        let marco = con(
            &mut escena,
            Figura::Marco {
                nombre: "hoja".into(),
            },
        );
        let fijo = rect(&mut escena);
        escena.buscar_mut(fijo).unwrap().bloqueado = true;
        let mut sel = Seleccion::nueva();
        sel.poner_todos([marco, fijo]);
        assert_eq!(pegar_a(&mut escena, &sel, &copiado), 0);
        assert!(
            propiedades_de(&Figura::Marco {
                nombre: String::new()
            })
            .is_empty()
        );
    }

    #[test]
    fn la_tabla_de_tipos_dice_lo_mismo_que_la_del_movil_donde_importa() {
        use Propiedad::*;
        // Las tres diferencias que el movil documenta una por una, y que son
        // las que se copian mal si la tabla se escribe de memoria.
        assert!(
            !propiedades_de(&Figura::Elipse).contains(&Esquinas),
            "la elipse no tiene esquinas que redondear"
        );
        assert!(
            propiedades_de(&Figura::Linea { puntos: vec![] }).contains(&Fondo),
            "la linea cerrada sobre si misma se rellena"
        );
        assert!(
            !propiedades_de(&Figura::Flecha {
                puntos: vec![],
                punta_inicio: false,
                punta_fin: true
            })
            .contains(&Fondo),
            "la flecha no encierra nada"
        );
        assert!(
            !propiedades_de(&Figura::Region {
                contorno: vec![],
                huecos: vec![]
            })
            .contains(&Trazo),
            "el relleno de un hueco es solo mancha: su borde lo dibujan otros"
        );
        assert!(
            !propiedades_de(&Figura::Cota { puntos: vec![] }).contains(&Rugosidad),
            "una cota temblorosa no dice donde acaba la medida"
        );
    }
}
