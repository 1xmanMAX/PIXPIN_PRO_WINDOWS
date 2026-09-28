//! La pila de capturas, sin una sola linea de Windows.
//!
//! Aqui vive TODO lo que hay que decidir: cuando una captura se suma a la
//! tanda que ya hay abierta, cuando esa tanda se cierra y empieza otra,
//! cuales estan elegidas y que se acaba poniendo en el portapapeles. Las
//! ventanas, los temporizadores y el portapapeles de verdad viven en
//! `ventana.rs` y en `pixpin-codec`, y solo preguntan aqui.
//!
//! # Por que el tiempo llega como un numero y no como `Instant`
//!
//! Un `Instant` no se puede fabricar en una prueba: solo se puede pedir al
//! reloj. Probar «una captura pasados once segundos NO se apila» obligaria a
//! dormir once segundos de verdad en cada ejecucion de la suite. Con los
//! milisegundos como `u64` la prueba es instantanea y el lado de Windows
//! pasa `GetTickCount64()`, que es justo eso: milisegundos monotonos desde
//! el arranque, sin saltos por cambios de hora ni de zona horaria.

use std::path::PathBuf;

use pixpin_codec::ImagenRgba;
use pixpin_geom::Rect;

/// Cuantas capturas caben como mucho en una tanda.
///
/// No es una cifra de diseno sino un freno de memoria: cada captura guarda su
/// miniatura descomprimida, y una tanda sin tope acabaria siendo un usuario
/// que dejo el atajo pulsado. Al pasarse, la mas vieja sale por abajo: el
/// usuario esta mirando las ultimas, no la primera.
pub const TOPE_CAPTURAS: usize = 24;

/// Una captura que entro en la pila.
///
/// Guarda la ruta del PNG **ya escrito en disco**, no los bytes: el pegado
/// como ficheros (`CF_HDROP`) necesita ficheros de verdad, y tener la ruta
/// desde el primer momento evita el baile de «copiar ahora obliga a escribir
/// N ficheros ahora», que con diez capturas se nota al pulsar el boton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captura {
    /// El PNG en `<datos>\capturas\`. Absoluta: `CF_HDROP` rechaza las
    /// relativas, y con razon (ver `pixpin_codec::construir_hdrop`).
    pub ruta: PathBuf,
    /// Reducida para pintar el icono y el panel. La grande se relee del
    /// disco si hace falta; tener N capturas completas en memoria seria
    /// cientos de megas por una tanda larga.
    pub miniatura: ImagenRgba,
    /// Tamano de la captura de verdad, para el rotulo del panel.
    pub ancho: u32,
    pub alto: u32,
    /// Marcada en el panel. Nace marcada: lo normal es querer todas, y
    /// obligar a marcarlas una a una para el caso normal seria trabajo de
    /// mas en el camino que el usuario recorre siempre.
    pub elegida: bool,
}

/// Que paso al meter una captura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Efecto {
    /// Es la primera de una tanda nueva: la anterior, si la habia, se cerro.
    Empezada,
    /// Se sumo a la tanda que ya estaba abierta.
    Apilada,
}

/// En que esquina del monitor se pone el icono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Esquina {
    ArribaIzquierda,
    ArribaDerecha,
    AbajoIzquierda,
    #[default]
    AbajoDerecha,
}

impl Esquina {
    /// Del nombre que se escribe en el TOML. `None` si no es ninguno: quien
    /// llama decide si avisar o caer al valor por defecto.
    pub fn por_nombre(nombre: &str) -> Option<Esquina> {
        match nombre {
            "arriba-izquierda" => Some(Esquina::ArribaIzquierda),
            "arriba-derecha" => Some(Esquina::ArribaDerecha),
            "abajo-izquierda" => Some(Esquina::AbajoIzquierda),
            "abajo-derecha" => Some(Esquina::AbajoDerecha),
            _ => None,
        }
    }
}

/// Lo que hay que dejar en el portapapeles.
///
/// Son dos casos y no uno porque **con una sola captura el portapapeles se
/// tiene que comportar exactamente como antes de que existiera la pila**: un
/// mapa de bits suelto. Publicar ademas la ruta cambiaria el pegado de
/// siempre — WhatsApp y Word prefieren el fichero cuando lo hay, y el usuario
/// veria aparecer un adjunto donde antes salia la imagen incrustada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copia {
    /// Una sola: solo el mapa de bits, como toda la vida.
    Imagen { ruta: PathBuf },
    /// Dos o mas: la lista de ficheros para que se peguen las N, y ademas la
    /// imagen de la ULTIMA para las aplicaciones que solo aceptan un mapa de
    /// bits (el cuadro de «pegar imagen» de muchas paginas web, por ejemplo).
    Ficheros {
        rutas: Vec<PathBuf>,
        imagen: PathBuf,
    },
}

impl Copia {
    /// Las rutas implicadas, en orden. Util para registrar y para probar.
    pub fn rutas(&self) -> &[PathBuf] {
        match self {
            Copia::Imagen { ruta } => std::slice::from_ref(ruta),
            Copia::Ficheros { rutas, .. } => rutas,
        }
    }

    /// La captura cuyo mapa de bits viaja suelto.
    pub fn imagen(&self) -> &PathBuf {
        match self {
            Copia::Imagen { ruta } => ruta,
            Copia::Ficheros { imagen, .. } => imagen,
        }
    }
}

/// La tanda de capturas que se esta juntando.
#[derive(Debug)]
pub struct Pila {
    capturas: Vec<Captura>,
    /// Milisegundos en que entro la ULTIMA de la tanda. Nacio contando desde
    /// la primera, y la primera prueba del usuario lo tumbo: cada recorte
    /// lleva dos o tres segundos de arrastrar, asi que en diez segundos desde
    /// la primera solo cabian dos y la tercera empezaba tanda nueva («solo me
    /// sale 1»). Contando desde la ultima, la tanda dura mientras se siga
    /// capturando y se cierra tras la ventana entera sin capturar; que no sea
    /// eterna lo garantiza `TOPE_CAPTURAS`.
    ultima_ms: Option<u64>,
    /// Ventana de apilado en milisegundos. Cero = apilar apagado.
    ventana_ms: u64,
    /// El monitor donde se hizo la PRIMERA de la tanda, en pixeles fisicos
    /// del escritorio virtual, y su escala. La tanda entera vive en ese
    /// monitor aunque las siguientes capturas se hagan en el otro: mover el
    /// icono de pantalla a media tanda es justo lo que hace perderlo de vista.
    monitor: Option<(Rect, u32)>,
}

impl Pila {
    /// `ventana_segundos` a cero apaga el apilado: cada captura cierra la
    /// anterior y la pila nunca tiene mas de una.
    pub fn nueva(ventana_segundos: u32) -> Pila {
        Pila {
            capturas: Vec::new(),
            ultima_ms: None,
            ventana_ms: ventana_segundos as u64 * 1000,
            monitor: None,
        }
    }

    pub fn apilar_encendido(&self) -> bool {
        self.ventana_ms > 0
    }

    pub fn capturas(&self) -> &[Captura] {
        &self.capturas
    }

    pub fn cuantas(&self) -> usize {
        self.capturas.len()
    }

    pub fn vacia(&self) -> bool {
        self.capturas.is_empty()
    }

    /// El monitor y la escala donde se planto la tanda. `None` con la pila
    /// vacia.
    pub fn monitor(&self) -> Option<(Rect, u32)> {
        self.monitor
    }

    /// La ultima que entro, que es la que se ve arriba del montoncito.
    pub fn ultima(&self) -> Option<&Captura> {
        self.capturas.last()
    }

    /// Mete una captura. `monitor` es el area fisica del monitor donde se
    /// hizo y `escala_por_cien` su escalado (150 en el monitor del usuario).
    pub fn anadir(
        &mut self,
        ahora_ms: u64,
        captura: Captura,
        monitor: Rect,
        escala_por_cien: u32,
    ) -> Efecto {
        let efecto = if self.sigue_abierta(ahora_ms) {
            Efecto::Apilada
        } else {
            // Cerrar es tirar lo anterior: la tanda vieja ya se pego o ya se
            // ignoro, y mezclarla con la nueva daria un Ctrl+V con capturas
            // de hace media hora.
            self.capturas.clear();
            self.monitor = Some((monitor, escala_por_cien));
            Efecto::Empezada
        };
        // Cada captura rearma la ventana, tambien las apiladas.
        self.ultima_ms = Some(ahora_ms);
        self.capturas.push(captura);
        if self.capturas.len() > TOPE_CAPTURAS {
            self.capturas.remove(0);
        }
        efecto
    }

    /// Si la tanda abierta todavia admite mas capturas en `ahora_ms`.
    fn sigue_abierta(&self, ahora_ms: u64) -> bool {
        if self.ventana_ms == 0 || self.capturas.is_empty() {
            return false;
        }
        match self.ultima_ms {
            // Un reloj monotono no retrocede, pero `saturating_sub` evita que
            // un salto raro (una maquina virtual suspendida, por ejemplo) se
            // convierta en un desbordamiento en vez de en una tanda cerrada.
            Some(ultima) => ahora_ms.saturating_sub(ultima) <= self.ventana_ms,
            None => false,
        }
    }

    /// Milisegundos que faltan para que la tanda deje de admitir capturas.
    /// `None` si no hay tanda abierta o el apilado esta apagado.
    ///
    /// Es lo que arma el UNICO temporizador que llega a existir: sin pila no
    /// hay temporizador, y por eso sin pila el consumo es cero.
    pub fn falta_para_cerrar_ms(&self, ahora_ms: u64) -> Option<u64> {
        if self.ventana_ms == 0 || self.capturas.is_empty() {
            return None;
        }
        let ultima = self.ultima_ms?;
        let pasado = ahora_ms.saturating_sub(ultima);
        Some(self.ventana_ms.saturating_sub(pasado))
    }

    /// Marca o desmarca la captura `i`. Devuelve si quedo marcada; `None` si
    /// `i` no existe (un clic en el panel puede llegar tarde, con la captura
    /// ya quitada).
    pub fn alternar(&mut self, i: usize) -> Option<bool> {
        let c = self.capturas.get_mut(i)?;
        c.elegida = !c.elegida;
        Some(c.elegida)
    }

    /// Quita una captura suelta de la pila. NO borra el PNG del disco: la
    /// captura sigue guardada en la carpeta de capturas como cualquier otra,
    /// y quitarla de la pila solo dice «esta no la voy a pegar».
    pub fn quitar(&mut self, i: usize) -> Option<Captura> {
        if i >= self.capturas.len() {
            return None;
        }
        let fuera = self.capturas.remove(i);
        if self.capturas.is_empty() {
            self.ultima_ms = None;
            self.monitor = None;
        }
        Some(fuera)
    }

    /// Vacia la pila entera. Tampoco borra ficheros, por lo mismo.
    pub fn vaciar(&mut self) {
        self.capturas.clear();
        self.ultima_ms = None;
        self.monitor = None;
    }

    /// Que copiar. `solo_elegidas` es el boton «Copiar las elegidas»; con
    /// `false` van todas («Copiar todas», y el Ctrl+V de siempre).
    ///
    /// `None` si no queda nada que copiar: publicar una lista vacia vaciaria
    /// el portapapeles del usuario a cambio de nada.
    pub fn que_copiar(&self, solo_elegidas: bool) -> Option<Copia> {
        let rutas: Vec<PathBuf> = self
            .capturas
            .iter()
            .filter(|c| !solo_elegidas || c.elegida)
            .map(|c| c.ruta.clone())
            .collect();
        match rutas.len() {
            0 => None,
            1 => Some(Copia::Imagen {
                ruta: rutas.into_iter().next().expect("largo comprobado"),
            }),
            _ => {
                // La ULTIMA es la que se acaba de hacer: es la que el usuario
                // tiene en la cabeza si el destino solo acepta una imagen.
                let imagen = rutas.last().expect("largo comprobado").clone();
                Some(Copia::Ficheros { rutas, imagen })
            }
        }
    }
}

/// Donde va el icono dentro de un monitor, en pixeles fisicos del escritorio
/// virtual.
///
/// `lado_logico` y `margen_logico` van en pixeles logicos (los de 100 %) y se
/// escalan aqui: al 150 % el icono tiene que salir medio mas grande o se vera
/// como un sello diminuto en el monitor del usuario.
///
/// Es una funcion suelta y pura a proposito: es justo el calculo que se
/// equivoca en el monitor de la izquierda (x negativa) y con el escalado, y
/// asi se puede probar sin encender una pantalla.
pub fn rect_del_icono(
    monitor: Rect,
    escala_por_cien: u32,
    esquina: Esquina,
    lado_logico: u32,
    margen_logico: u32,
) -> Rect {
    let escalar = |v: u32| ((v as u64 * escala_por_cien.max(100) as u64) / 100) as u32;
    let lado = escalar(lado_logico);
    let margen = escalar(margen_logico);
    // Un icono mas grande que el monitor no se pega a ninguna esquina; se
    // encoge antes de decidir, para que las cuentas de abajo no den negativo.
    let lado = lado.min(monitor.ancho).min(monitor.alto);
    let izquierda = monitor.x + margen as i32;
    let arriba = monitor.y + margen as i32;
    let derecha = monitor.x + monitor.ancho as i32 - lado as i32 - margen as i32;
    let abajo = monitor.y + monitor.alto as i32 - lado as i32 - margen as i32;
    let (x, y) = match esquina {
        Esquina::ArribaIzquierda => (izquierda, arriba),
        Esquina::ArribaDerecha => (derecha, arriba),
        Esquina::AbajoIzquierda => (izquierda, abajo),
        Esquina::AbajoDerecha => (derecha, abajo),
    };
    // Con margenes grandes en un monitor pequeno los dos bordes se cruzan;
    // pegarse al borde es mejor que salirse de la pantalla.
    Rect {
        x: x.max(monitor.x).min(monitor.x + monitor.ancho as i32 - lado as i32),
        y: y.max(monitor.y).min(monitor.y + monitor.alto as i32 - lado as i32),
        ancho: lado,
        alto: lado,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn captura(nombre: &str) -> Captura {
        Captura {
            ruta: PathBuf::from(format!(r"C:\datos\capturas\{nombre}.png")),
            miniatura: ImagenRgba {
                ancho: 1,
                alto: 1,
                pixeles: vec![0, 0, 0, 255],
            },
            ancho: 800,
            alto: 600,
            elegida: true,
        }
    }

    fn monitor() -> Rect {
        Rect {
            x: 0,
            y: 0,
            ancho: 1920,
            alto: 1080,
        }
    }

    #[test]
    fn dos_capturas_seguidas_se_apilan_en_la_misma_tanda() {
        let mut p = Pila::nueva(10);
        assert_eq!(
            p.anadir(1_000, captura("a"), monitor(), 100),
            Efecto::Empezada
        );
        assert_eq!(
            p.anadir(4_000, captura("b"), monitor(), 100),
            Efecto::Apilada
        );
        assert_eq!(p.cuantas(), 2);
    }

    #[test]
    fn una_captura_pasado_el_tiempo_no_se_apila_y_empieza_otra_tanda() {
        // El caso negativo que da sentido a toda la funcion: si esto fallara,
        // el Ctrl+V de la tarde arrastraria las capturas de la manana.
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        assert_eq!(
            p.anadir(10_001, captura("b"), monitor(), 100),
            Efecto::Empezada,
            "pasados los diez segundos la tanda vieja se cierra"
        );
        assert_eq!(p.cuantas(), 1, "la nueva tanda empieza limpia");
        assert_eq!(p.ultima().map(|c| c.ruta.clone()), Some(captura("b").ruta));
    }

    #[test]
    fn el_tiempo_se_cuenta_desde_la_ultima_y_no_desde_la_primera() {
        // Lo que fallo en la primera prueba del usuario: tres recortes
        // seguidos, el tercero a trece segundos del primero, y salia «1».
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        p.anadir(8_500, captura("b"), monitor(), 100);
        assert_eq!(
            p.anadir(13_300, captura("c"), monitor(), 100),
            Efecto::Apilada,
            "trece segundos despues de la primera, pero cinco de la ultima"
        );
        assert_eq!(p.cuantas(), 3);
        // Caso negativo: diez segundos largos SIN capturar si cierran.
        assert_eq!(
            p.anadir(23_301, captura("d"), monitor(), 100),
            Efecto::Empezada
        );
        assert_eq!(p.cuantas(), 1);
    }

    #[test]
    fn justo_en_el_borde_de_la_ventana_todavia_se_apila() {
        let mut p = Pila::nueva(10);
        p.anadir(500, captura("a"), monitor(), 100);
        assert_eq!(
            p.anadir(10_500, captura("b"), monitor(), 100),
            Efecto::Apilada,
            "el segundo diez exacto cuenta como dentro"
        );
    }

    #[test]
    fn con_el_apilado_apagado_cada_captura_empieza_su_propia_tanda() {
        // `apilar_segundos = 0` en el TOML tiene que dejar el comportamiento
        // de siempre: una captura, una imagen en el portapapeles.
        let mut p = Pila::nueva(0);
        assert!(!p.apilar_encendido());
        p.anadir(0, captura("a"), monitor(), 100);
        assert_eq!(p.anadir(1, captura("b"), monitor(), 100), Efecto::Empezada);
        assert_eq!(p.cuantas(), 1);
        assert_eq!(p.falta_para_cerrar_ms(1), None, "ni un temporizador");
    }

    #[test]
    fn una_pila_vacia_no_copia_nada() {
        // Caso negativo: publicar una lista vacia vaciaria el portapapeles
        // del usuario sin dejar nada a cambio.
        let p = Pila::nueva(10);
        assert_eq!(p.que_copiar(false), None);
        assert_eq!(p.que_copiar(true), None);
        assert_eq!(p.falta_para_cerrar_ms(0), None);
        assert_eq!(p.monitor(), None);
    }

    #[test]
    fn con_una_sola_captura_el_portapapeles_lleva_solo_la_imagen() {
        // El comportamiento de antes de que existiera la pila, intacto.
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        assert_eq!(
            p.que_copiar(false),
            Some(Copia::Imagen {
                ruta: captura("a").ruta
            })
        );
    }

    #[test]
    fn con_varias_van_los_ficheros_y_la_imagen_de_la_ultima() {
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        p.anadir(1_000, captura("b"), monitor(), 100);
        p.anadir(2_000, captura("c"), monitor(), 100);
        let copia = p.que_copiar(false).expect("hay tres");
        assert_eq!(copia.rutas().len(), 3);
        assert_eq!(copia.imagen(), &captura("c").ruta, "la ultima que se hizo");
    }

    #[test]
    fn copiar_las_elegidas_deja_fuera_las_desmarcadas() {
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        p.anadir(1_000, captura("b"), monitor(), 100);
        p.anadir(2_000, captura("c"), monitor(), 100);
        assert_eq!(p.alternar(1), Some(false), "la b se desmarca");
        let copia = p.que_copiar(true).expect("quedan dos");
        assert_eq!(
            copia.rutas(),
            [captura("a").ruta, captura("c").ruta],
            "y la b no viaja"
        );
        assert_eq!(
            p.que_copiar(false).map(|c| c.rutas().len()),
            Some(3),
            "«copiar todas» sigue llevandose las tres"
        );
    }

    #[test]
    fn desmarcarlas_todas_no_copia_nada() {
        // Caso negativo del boton «Copiar las elegidas» sin nada elegido.
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        p.alternar(0);
        assert_eq!(p.que_copiar(true), None);
    }

    #[test]
    fn alternar_o_quitar_una_que_no_existe_no_revienta() {
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        assert_eq!(p.alternar(7), None);
        assert_eq!(p.quitar(7), None);
        assert_eq!(p.cuantas(), 1, "y la que habia sigue ahi");
    }

    #[test]
    fn quitar_la_ultima_deja_la_pila_como_recien_nacida() {
        let mut p = Pila::nueva(10);
        p.anadir(1_000, captura("a"), monitor(), 100);
        assert!(p.quitar(0).is_some());
        assert!(p.vacia());
        assert_eq!(p.monitor(), None);
        // Y la siguiente empieza tanda, no se cuela en la que estaba abierta.
        assert_eq!(
            p.anadir(1_100, captura("b"), monitor(), 100),
            Efecto::Empezada
        );
    }

    #[test]
    fn la_pila_no_crece_sin_fin() {
        let mut p = Pila::nueva(3600);
        for i in 0..(TOPE_CAPTURAS + 5) {
            p.anadir(i as u64, captura(&format!("c{i}")), monitor(), 100);
        }
        assert_eq!(p.cuantas(), TOPE_CAPTURAS);
        assert_eq!(
            p.ultima().map(|c| c.ruta.clone()),
            Some(captura(&format!("c{}", TOPE_CAPTURAS + 4)).ruta),
            "la ultima que entro sigue siendo la de arriba"
        );
    }

    #[test]
    fn lo_que_falta_para_cerrar_va_bajando_y_llega_a_cero() {
        let mut p = Pila::nueva(10);
        p.anadir(1_000, captura("a"), monitor(), 100);
        assert_eq!(p.falta_para_cerrar_ms(1_000), Some(10_000));
        assert_eq!(p.falta_para_cerrar_ms(7_000), Some(4_000));
        assert_eq!(p.falta_para_cerrar_ms(50_000), Some(0), "no se pasa de rosca");
    }

    #[test]
    fn otra_captura_rearma_lo_que_falta_para_cerrar() {
        // Es lo que rearma el desvanecido del icono: si no subiera, el icono
        // se iria de la esquina con la tanda todavia abierta.
        let mut p = Pila::nueva(10);
        p.anadir(1_000, captura("a"), monitor(), 100);
        p.anadir(7_000, captura("b"), monitor(), 100);
        assert_eq!(p.falta_para_cerrar_ms(7_000), Some(10_000));
    }

    #[test]
    fn la_tanda_se_queda_en_el_monitor_de_la_primera_captura() {
        // El usuario tiene dos monitores; si la segunda captura la hace en el
        // otro, mover el icono de pantalla es perderlo de vista.
        let otro = Rect {
            x: 1920,
            y: 0,
            ancho: 2560,
            alto: 1440,
        };
        let mut p = Pila::nueva(10);
        p.anadir(0, captura("a"), monitor(), 100);
        p.anadir(1_000, captura("b"), otro, 150);
        assert_eq!(p.monitor(), Some((monitor(), 100)));
    }

    #[test]
    fn el_icono_se_pega_a_la_esquina_que_diga_el_ajuste() {
        let m = monitor();
        let r = rect_del_icono(m, 100, Esquina::AbajoDerecha, 96, 24);
        assert_eq!(
            (r.x, r.y, r.ancho, r.alto),
            (1920 - 96 - 24, 1080 - 96 - 24, 96, 96)
        );
        let r = rect_del_icono(m, 100, Esquina::ArribaIzquierda, 96, 24);
        assert_eq!((r.x, r.y), (24, 24));
    }

    #[test]
    fn al_ciento_cincuenta_por_ciento_el_icono_sale_medio_mas_grande() {
        // El monitor del usuario. Sin esto el icono seria un sello.
        let r = rect_del_icono(monitor(), 150, Esquina::AbajoDerecha, 96, 24);
        assert_eq!((r.ancho, r.alto), (144, 144));
        assert_eq!((r.x, r.y), (1920 - 144 - 36, 1080 - 144 - 36));
    }

    #[test]
    fn en_el_monitor_de_la_izquierda_la_x_sale_negativa() {
        // Caso negativo clasico del escritorio virtual: el monitor secundario
        // a la izquierda del principal tiene coordenadas negativas, y una
        // cuenta hecha sobre el tamano y no sobre el origen pondria el icono
        // en el monitor equivocado.
        let izq = Rect {
            x: -1920,
            y: 0,
            ancho: 1920,
            alto: 1080,
        };
        let r = rect_del_icono(izq, 100, Esquina::AbajoIzquierda, 96, 24);
        assert_eq!((r.x, r.y), (-1920 + 24, 1080 - 96 - 24));
    }

    #[test]
    fn un_icono_mas_grande_que_el_monitor_no_se_sale() {
        let chico = Rect {
            x: 0,
            y: 0,
            ancho: 80,
            alto: 60,
        };
        let r = rect_del_icono(chico, 100, Esquina::AbajoDerecha, 96, 24);
        assert!(r.x >= 0 && r.y >= 0, "no se sale por arriba ni por la izquierda");
        assert!(
            r.x + r.ancho as i32 <= 80 && r.y + r.alto as i32 <= 60,
            "ni por abajo ni por la derecha"
        );
    }

    #[test]
    fn las_esquinas_se_leen_del_toml_y_una_mal_escrita_no_cuela() {
        assert_eq!(
            Esquina::por_nombre("abajo-derecha"),
            Some(Esquina::AbajoDerecha)
        );
        assert_eq!(
            Esquina::por_nombre("arriba-izquierda"),
            Some(Esquina::ArribaIzquierda)
        );
        // Caso negativo: un nombre inventado no se parece al mas cercano.
        assert_eq!(Esquina::por_nombre("abajo derecha"), None);
        assert_eq!(Esquina::por_nombre(""), None);
        assert_eq!(Esquina::default(), Esquina::AbajoDerecha);
    }
}
