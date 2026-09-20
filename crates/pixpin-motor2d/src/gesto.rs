//! Todo lo que pasa entre pulsar y soltar.
//!
//! En el Android son 3.482 lineas (`DrawController.kt`) porque atiende a
//! dos, tres y cuatro dedos, al lapiz y al canto de la mano. Aqui hay un
//! raton y un teclado, asi que donde el Android pregunta «hay segundo
//! dedo?», esto pregunta «esta Shift?».
//!
//! # Que toca y que devuelve (D23)
//!
//! **La escena si la toca.** Es un `Vec` de datos, se prueba sin pantalla, y
//! mantener aparte un registro de ordenes pendientes seria una segunda
//! verdad sobre el mismo dibujo.
//!
//! **Lo que devuelve es lo que la ventana tiene que hacer**: que region
//! redibujar y que cursor poner. De Direct2D no sabe nada. Por eso esto vive
//! en el motor y no en la interfaz: una prueba puede decir «pulsar aqui,
//! mover cuarenta pixeles con Shift, soltar» y comprobar el resultado exacto
//! sin abrir una ventana.
//!
//! # El orden de decision al pulsar
//!
//! Es la parte que hay que leer despacio:
//!
//! 1. **Un tirador manda sobre lo que haya debajo.** Sin esto, un tirador
//!    encima de un trazo es inalcanzable.
//! 2. **Lo ya seleccionado manda sobre lo de encima.** Sin esto, mover un
//!    grupo se convierte en seleccionar por accidente lo que estaba encima.
//! 3. Lo que haya bajo el cursor.
//! 4. Vacio: marquesina si la herramienta es la mano, dibujar si no.

use pixpin_geom::Tirador;

use crate::elemento::{Elemento, Figura};
use crate::escena::Escena;
use crate::impacto::{dentro_de, elemento_en};
use crate::medida::Escala;
use crate::seleccion::Seleccion;
use crate::tiradores::{Agarre, Tiradores};
use crate::transformar::{self, a_saltos, angulo_hacia};
use crate::vector::Punto2;

/// Puntos que se reservan de una vez para el trazo en curso.
///
/// 512 porque el trazo mas largo del fichero del movil tiene 492. Reservar
/// de una vez es lo que permite que mover el raton dibujando no asigne
/// memoria — la regla del camino caliente.
pub const PUNTOS_RESERVADOS: usize = 512;

/// El punto final restringido a un angulo redondo, para Shift-arrastrar.
///
/// El giro medio -de 0 a 180 grados- se reparte en `pasos` tramos iguales:
/// con 12 salen horizontal, vertical y las dos diagonales cada 15 grados
/// (Linea y Flecha, en `pixpin-ui/anotador.rs`); con 2 solo salen horizontal
/// y vertical (`Calibrando` aqui abajo: calibrar sobre el borde de una
/// pared es el caso normal, y a pulso sale torcida -un grado de mas son dos
/// centimetros de error por metro-).
///
/// Publica y aqui, en el motor, y no repetida en `pixpin-ui`: es logica
/// pura sin nada de interfaz, y `pixpin-ui` ya depende de este crate.
pub fn restringir_angulo(inicio: Punto2, fin: Punto2, pasos: f32) -> Punto2 {
    let (dx, dy) = (fin.x - inicio.x, fin.y - inicio.y);
    let radio = (dx * dx + dy * dy).sqrt();
    if radio == 0.0 {
        return fin;
    }
    let paso = std::f32::consts::PI / pasos;
    let angulo = (dy.atan2(dx) / paso).round() * paso;
    Punto2 {
        x: inicio.x + radio * angulo.cos(),
        y: inicio.y + radio * angulo.sin(),
    }
}

/// Con que se dibuja. Vino de `pixpin-ui/anotador.rs` en el paso 1: quien
/// decide que hace un clic tiene que saber que herramienta hay puesta, y esa
/// decision es logica pura, no interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Herramienta {
    /// Seleccionar y mover lo ya dibujado.
    Mano,
    Lapiz,
    Resaltador,
    Linea,
    Flecha,
    Rectangulo,
    Elipse,
    Texto,
    /// Oscurece todo menos una zona (D51).
    Foco,
    /// Amplia alrededor del cursor. No deja rastro: es una vista (D52).
    Lupa,
    Borrador,
    /// Acota: deja una raya que dice cuanto mide.
    Cota,
    /// Calibra: se arrastra sobre algo de medida conocida y al soltar
    /// pregunta cuanto mide de verdad. La raya no se guarda.
    Escalar,
    /// La reglita a cuadros que sobrevive a la fotocopia.
    EscalaGrafica,
    /// Un marco: recuadro con nombre que se lleva consigo lo que encierra.
    Marco,
    /// Un emoji suelto (universo). El gesto no crea nada con ella: quien
    /// sabe que emoji se eligio es la sesion del universo, y es ella la que
    /// lo coloca en la escena.
    Emoji,

    // --- Las que abre la tanda cero ---
    //
    // Estan aqui, en un solo sitio y de una vez, para que los cuatro grupos
    // que vienen a portar herramientas no tengan que volver a tocar este
    // enumerado ni el `match` central: cada uno rellena su modulo y ya.
    // Todas producen ALGO honesto desde el primer dia —una figura que se ve
    // y que viaja al movil— aunque el detalle fino sea de su grupo.
    /// El rombo de Excalidraw. Una de sus diez figuras principales.
    Rombo,
    /// Un trozo de ovalo: se pone la guia y se repasa (grupo A).
    Arco,
    /// La flecha de codos, el conector de organigrama (grupo A). Hasta que
    /// su grupo porte `Elbow.kt` nace como una flecha recta: se ve, se
    /// mueve y llega al movil como flecha, que es lo peor que puede pasar.
    FlechaCodos,
    /// La flecha a pulso: por dentro ES una flecha, con todos los puntos
    /// del trazo en vez de dos (`Scene.kt:57-64`).
    FlechaLibre,
    /// Selecciona con un contorno a mano (grupo B).
    Lazo,
    /// Tapa lo que hay debajo (grupo D).
    Mosaico,
    /// El circulito numerado de anotar una captura paso a paso (grupo D).
    Serie,
    /// El bote: rellena el hueco entre varias figuras (grupo C).
    Relleno,
    /// Quita el trozo de raya que sobra hasta donde la cruzan las demas
    /// (grupo C).
    Recortar,
    /// Alarga la punta que se queda corta hasta lo primero que topa
    /// (grupo C).
    Extender,
    /// Un punto con su letra, para un croquis de geometria (grupo C).
    Punto,
    /// Copia el estilo de una figura y lo pega en otra (grupo B). Es
    /// invencion de escritorio: en el movil no existe.
    CopiarEstilo,
}

impl Herramienta {
    /// Si necesita un arrastre de verdad para producir algo. El lapiz no:
    /// un clic deja un punto de tinta, que es lo que espera cualquiera que
    /// haya usado un rotulador.
    pub fn necesita_arrastre(self) -> bool {
        !matches!(
            self,
            Herramienta::Lapiz
                | Herramienta::Texto
                // La flecha a pulso se traza como el lapiz, punto a punto.
                | Herramienta::FlechaLibre
                // Estas tres se plantan de un toque: el punto no tiene caja
                // que arrastrar y el numero de serie tampoco.
                | Herramienta::Serie
                | Herramienta::Punto
        )
    }

    /// Si lo que dibuja se guarda en el documento.
    ///
    /// Las que dicen que no son de dos clases: las que solo MIRAN (la mano,
    /// la lupa-vista) y **las que trabajan sobre lo que ya hay** —el bote,
    /// recortar, extender, copiar estilo, el lazo—. Estas ultimas si tocan
    /// el documento, pero no naciendo un elemento bajo el arrastre, que es
    /// lo unico que esta bandera decide.
    pub fn deja_rastro(self) -> bool {
        !matches!(
            self,
            Herramienta::Mano
                | Herramienta::Lupa
                | Herramienta::Borrador
                | Herramienta::Escalar
                | Herramienta::Lazo
                | Herramienta::Relleno
                | Herramienta::Recortar
                | Herramienta::Extender
                | Herramienta::CopiarEstilo
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventoGesto {
    /// `presion` es la del lapiz (0-1) o `None` con raton.
    Pulsar {
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
    },
    Mover {
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
    },
    Soltar {
        p: Punto2,
    },
    Escape,
    Suprimir,
    Deshacer,
    Rehacer,
    SeleccionarTodo,
}

/// Que hay que volver a pintar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Region {
    Nada,
    /// Solo esta caja del mundo. Es lo que se usa en el camino caliente.
    Caja(f32, f32, f32, f32),
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormaCursor {
    Flecha,
    Cruz,
    Mover,
    Texto,
    Giro,
    /// Escalar, con el tirador (para saber a que lado apunta) y el angulo
    /// del elemento (para girar esa direccion con el).
    Escalar {
        tirador: Tirador,
        angulo: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Respuesta {
    pub region: Region,
    pub cursor: FormaCursor,
    pub pide: Option<Peticion>,
}

/// Algo que la maquina necesita y solo la ventana puede conseguir.
///
/// La maquina dice «hay que preguntar esto»; quien pregunta y como es asunto
/// del que pinta (D40). Mismo corte que `Region` y `FormaCursor`: aqui no se
/// sabe lo que es una ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Peticion {
    /// Cuanto mide de verdad el trazo que se acaba de arrastrar.
    Calibrar { largo_px: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Estado {
    Reposo,
    Dibujando {
        id: u64,
    },
    Moviendo {
        anterior: Punto2,
    },
    Escalando {
        tirador: Tirador,
    },
    Girando {
        anterior: f32,
    },
    Marquesina {
        origen: Punto2,
        hasta: Punto2,
    },
    /// Arrastrando la raya de `Escalar`. No hay id: no deja rastro, y sus
    /// puntos van en `trazo` en vez de en un elemento de la escena.
    Calibrando,
}

pub struct Gesto {
    estado: Estado,
    pub seleccion: Seleccion,
    pub herramienta: Herramienta,
    /// El trazo en curso. Se reserva una vez y se reutiliza con `clear()`.
    trazo: Vec<Punto2>,
    /// A que se pega el cursor. Lo escribe la ventana desde los ajustes
    /// guardados; por defecto, encendido con todas las clases de punto.
    pub enganche: crate::enganche::Ajustes,
    /// El ancla a la que se ha pegado el ultimo punto, para que la ventana
    /// pinte la pista. Se apaga al soltar y al cancelar: una marca que
    /// sobrevive al gesto es una marca mintiendo.
    pub anclaje_activo: Option<crate::enganche::Anclaje>,
    /// El `strokeWidth` de Excalidraw para el lapiz: 0,5 / 1 / 2.
    pub grosor_tinta: f32,
    /// Pluma variable o constante para los trazos nuevos.
    pub variabilidad: crate::tinta::Variabilidad,
    /// Una elipse de forma rapida que se esta ajustando: centro, distancia
    /// del cursor al convertirla y radios de entonces. Escala desde su
    /// centro; crecer desde una esquina la haria saltar al convertirse.
    forma_elipse: Option<(Punto2, f32, f32, f32)>,
    /// El «actual» del panel lateral: con esto nacen las figuras nuevas.
    /// El grosor del lapiz sigue en `grosor_tinta` (las teclas 1/2/3).
    pub estilo: crate::estilo::EstiloDibujo,
    /// El texto que se esta escribiendo: que elemento es y como va su
    /// cursor. Mientras esto tiene algo, las teclas son del texto y no
    /// atajos de herramienta.
    pub escribiendo: Option<(u64, crate::texto::EdicionTexto)>,
}

impl Default for Gesto {
    fn default() -> Self {
        Self {
            estado: Estado::Reposo,
            seleccion: Seleccion::nueva(),
            herramienta: Herramienta::Lapiz,
            trazo: Vec::with_capacity(PUNTOS_RESERVADOS),
            enganche: crate::enganche::Ajustes::default(),
            anclaje_activo: None,
            grosor_tinta: crate::tinta::GROSOR_MEDIO,
            variabilidad: crate::tinta::Variabilidad::Variable,
            estilo: crate::estilo::EstiloDibujo::default(),
            escribiendo: None,
            forma_elipse: None,
        }
    }
}

impl Gesto {
    pub fn nuevo() -> Self {
        Self::default()
    }

    pub fn en_reposo(&self) -> bool {
        matches!(self.estado, Estado::Reposo)
    }

    /// El trazo a mano que se esta dibujando ahora. La ventana lo excluye de
    /// la capa congelada (D120): es lo unico que cambia en cada fotograma.
    pub fn trazo_en_curso(&self) -> Option<u64> {
        match self.estado {
            Estado::Dibujando { id }
                if matches!(
                    self.herramienta,
                    Herramienta::Lapiz | Herramienta::Resaltador
                ) =>
            {
                Some(id)
            }
            _ => None,
        }
    }

    /// Forma rapida: si se esta dibujando a mano y el trazo parece una linea,
    /// un rectangulo o una elipse, lo convierte en esa figura (con su color y
    /// un grosor equivalente) y deja listo el ajuste: los movimientos
    /// siguientes la redimensionan. Lo llama la ventana cuando el cursor se
    /// queda quieto con el boton pulsado. `None` si no hay nada que convertir.
    /// Sigue siendo el mismo elemento, anadido en este paso: un Ctrl+Z lo
    /// quita entero.
    pub fn convertir_en_forma(&mut self, escena: &mut Escena, cursor: Punto2) -> Option<Respuesta> {
        use crate::forma_rapida::{FormaReconocida, reconocer};
        let Estado::Dibujando { id } = self.estado else {
            return None;
        };
        if self.herramienta != Herramienta::Lapiz {
            return None;
        }
        let e = escena.buscar(id)?;
        let Figura::Lapiz { puntos, .. } = &e.figura else {
            return None;
        };
        let forma = reconocer(puntos)?;
        let grosor = crate::estilo::NivelGrosor::de_elemento(&e.figura, e.grosor).de_forma();
        let relleno = self.estilo.relleno;
        let e = escena.buscar_mut(id)?;
        e.grosor = grosor;
        e.angulo = 0.0;
        self.forma_elipse = None;
        match forma {
            FormaReconocida::Linea { a, .. } => {
                e.figura = Figura::Linea {
                    puntos: vec![a, cursor],
                };
                e.x = a.x;
                e.y = a.y;
                self.trazo.clear();
                self.trazo.push(a);
            }
            FormaReconocida::Rectangulo { caja } => {
                e.figura = Figura::Rectangulo;
                e.relleno = relleno;
                (e.x, e.y, e.ancho, e.alto) = (caja.0, caja.1, caja.2 - caja.0, caja.3 - caja.1);
                // Crece desde la esquina opuesta al cursor: la que queda fija
                // mientras se tira de la otra.
                let esquinas = [
                    Punto2::nuevo(caja.0, caja.1),
                    Punto2::nuevo(caja.2, caja.1),
                    Punto2::nuevo(caja.0, caja.3),
                    Punto2::nuevo(caja.2, caja.3),
                ];
                let ancla = esquinas
                    .into_iter()
                    .max_by(|a, b| a.distancia(cursor).total_cmp(&b.distancia(cursor)))?;
                self.trazo.clear();
                self.trazo.push(ancla);
            }
            FormaReconocida::Elipse { caja } => {
                e.figura = Figura::Elipse;
                e.relleno = relleno;
                (e.x, e.y, e.ancho, e.alto) = (caja.0, caja.1, caja.2 - caja.0, caja.3 - caja.1);
                let centro = Punto2::nuevo((caja.0 + caja.2) / 2.0, (caja.1 + caja.3) / 2.0);
                self.forma_elipse = Some((
                    centro,
                    cursor.distancia(centro).max(1.0),
                    e.ancho / 2.0,
                    e.alto / 2.0,
                ));
            }
        }
        e.tocar();
        Some(Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Cruz,
            pide: None,
        })
    }

    /// Cualquier elemento que se esta dibujando (no solo el trazo a mano) y
    /// el punto donde se pulso: la ventana pinta su punta predicha.
    pub fn elemento_en_curso(&self) -> Option<(u64, Option<Punto2>)> {
        match self.estado {
            // La elipse rapida no crece desde un origen: sin origen, la punta
            // predicha no la deforma.
            Estado::Dibujando { id } if self.forma_elipse.is_some() => Some((id, None)),
            Estado::Dibujando { id } => Some((id, self.trazo.first().copied())),
            _ => None,
        }
    }

    /// La marquesina en curso, para que la ventana la pinte.
    pub fn marquesina(&self) -> Option<(f32, f32, f32, f32)> {
        match self.estado {
            Estado::Marquesina { origen, hasta } => Some((origen.x, origen.y, hasta.x, hasta.y)),
            _ => None,
        }
    }

    /// Para la prueba de que el buffer se reutiliza.
    pub fn capacidad_del_trazo(&self) -> usize {
        self.trazo.capacity()
    }

    pub fn evento(&mut self, ev: EventoGesto, escena: &mut Escena, escala: f32) -> Respuesta {
        match ev {
            // Aqui el punto va CRUDO, a proposito: `pulsar` criba con el
            // punto de verdad y solo engancha una vez decidido que la rama
            // es de trazo o de calibrar. Si se enganchara antes, todo
            // anclaje de esquina, extremo o medio -que caen justo sobre la
            // geometria del vecino- haria que el paso 3 de `pulsar`
            // (`elemento_en`) diera positivo, y el clic se convertiria en
            // «seleccionar el vecino» en vez de dibujar o calibrar.
            EventoGesto::Pulsar {
                p,
                shift,
                alt,
                presion,
            } => self.pulsar(p, shift, alt, presion, escena, escala),
            EventoGesto::Mover {
                p,
                shift,
                alt,
                presion,
            } => {
                let p = self.enganchar(p, escena, escala);
                self.mover(p, shift, alt, presion, escena, escala)
            }
            EventoGesto::Soltar { p } => {
                self.anclaje_activo = None;
                self.soltar(p, escena, escala)
            }
            EventoGesto::Escape => {
                escena.cancelar_paso();
                self.estado = Estado::Reposo;
                self.seleccion.limpiar();
                self.anclaje_activo = None;
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Suprimir => {
                escena.abrir_paso();
                for &id in self.seleccion.ids() {
                    // `borrar_apuntando` y no `borrar`: el primero apunta el
                    // cambio en el paso, el segundo no. Con `borrar`, la
                    // prueba `suprimir_borra_lo_seleccionado_de_una_vez`
                    // falla en su segunda mitad.
                    escena.borrar_apuntando(id);
                }
                escena.cerrar_paso();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Deshacer => {
                escena.deshacer();
                self.seleccion.limpiar();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::Rehacer => {
                escena.rehacer();
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
            EventoGesto::SeleccionarTodo => {
                let vivos: Vec<u64> = escena.visibles().map(|e| e.id).collect();
                self.seleccion.poner_todos(vivos);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }
        }
    }

    /// Que se esta haciendo, para que el iman sepa a que debe pegarse.
    ///
    /// Sale de la maquina de estados y no de la herramienta puesta, que es
    /// la idea entera de `Faena`: lo que decide el enganche es que esta
    /// pasando, no que boton hay pulsado.
    fn faena(&self) -> Option<crate::enganche::Faena> {
        use crate::enganche::Faena;
        match self.estado {
            // El punto que hace nacer una figura.
            Estado::Reposo => match self.herramienta {
                Herramienta::Lapiz | Herramienta::Resaltador => Some(Faena::AMano),
                // `Mano` selecciona y mueve; `Lupa` es una vista y no deja
                // rastro; el `Borrador` quita, no coloca. Ninguna de las
                // tres pone un punto que merezca engancharse.
                Herramienta::Mano | Herramienta::Lupa | Herramienta::Borrador => None,
                _ => Some(Faena::Trazando),
            },
            Estado::Dibujando { .. } => match self.herramienta {
                Herramienta::Lapiz | Herramienta::Resaltador => Some(Faena::AMano),
                _ => Some(Faena::Trazando),
            },
            // Calibrar es trazar una raya de dos puntos, y es donde mas
            // falta hace: el error de picar a pulso entra directo en la
            // escala y lo hereda todo lo que se mida despues.
            Estado::Calibrando => Some(Faena::Trazando),
            Estado::Moviendo { .. } => Some(Faena::Moviendo),
            Estado::Escalando { .. } | Estado::Girando { .. } => Some(Faena::Afinando),
            // Seleccionar no es dibujar: nada tira del cursor.
            Estado::Marquesina { .. } => None,
        }
    }

    /// El punto ya enganchado, y deja apuntado a que -para pintar la pista-.
    fn enganchar(&mut self, p: Punto2, escena: &Escena, escala: f32) -> Punto2 {
        let Some(faena) = self.faena() else {
            self.anclaje_activo = None;
            return p;
        };
        // `escala` son unidades de escena por pixel de pantalla -la ventana
        // pasa `1.0 / camara.zoom`-, y `sitio` quiere el zoom. Sin esta
        // vuelta el radio del iman sale invertido: acercarse lo haria
        // agarrar mas lejos.
        let zoom = 1.0 / escala.max(0.0001);
        // Lo que se esta dibujando o moviendo no cuenta: se engancharia a si
        // mismo en cuanto naciera.
        let propios: [u64; 1];
        let excluir: &[u64] = match self.estado {
            Estado::Dibujando { id } => {
                propios = [id];
                &propios
            }
            Estado::Moviendo { .. } => self.seleccion.ids(),
            _ => &[],
        };
        let encontrado =
            crate::enganche::sitio(&escena.elementos, p, zoom, faena, &self.enganche, excluir);
        self.anclaje_activo = encontrado;
        encontrado.map_or(p, |a| a.punto)
    }

    /// Pone la escala del lienzo a partir de una medida real.
    ///
    /// Devuelve si pudo: una calibracion imposible no toca la escena (D36).
    /// Va en un paso de deshacer porque calibrar mal y no poder volver atras
    /// seria tener que rehacer el lienzo entero.
    pub fn calibrar(
        &mut self,
        escena: &mut Escena,
        largo_px: f32,
        valor: f32,
        unidad: &str,
    ) -> bool {
        let Some(nueva) = Escala::calibrando(largo_px, valor, unidad, 2) else {
            return false;
        };
        escena.abrir_paso();
        escena.apuntar_escala();
        escena.escala = Some(nueva);
        escena.cerrar_paso();
        true
    }

    /// Los tiradores de la seleccion, si hay algo elegido.
    ///
    /// Publico a proposito: es la unica fuente de verdad sobre donde caen
    /// los tiradores. `cursor_en` y `pulsar` ya lo usaban para decidir que
    /// agarra el clic; quien los PINTA tiene que llamar a este mismo
    /// metodo y no recalcular el angulo por su cuenta, que es justo lo que
    /// paso una vez: los tiradores se pintaban rectos mientras el clic
    /// respondia girado.
    pub fn tiradores(&self, escena: &Escena, escala: f32) -> Option<Tiradores> {
        let caja = self.seleccion.caja(escena)?;
        // Con un solo elemento, el marco lleva su angulo. Con varios, la
        // caja es paralela a los ejes y cada uno conserva el suyo.
        let angulo = match self.seleccion.ids() {
            [uno] => escena.buscar(*uno).map_or(0.0, |e| e.angulo),
            _ => 0.0,
        };
        Some(Tiradores::de_caja(caja, angulo, escala))
    }

    /// El elemento nuevo que empieza esta herramienta en este punto.
    ///
    /// Los puntos se reservan de una vez, igual que el buffer del gesto: es
    /// lo que hace que mover el raton dibujando **no asigne memoria**. Si
    /// este `Vec` empezara vacio, crecer de 4 a 8 a 16... asignaria una
    /// docena de veces por trazo, en el unico camino del programa con un
    /// plazo sagrado. La tarea 15 lo comprueba contando asignaciones.
    fn nuevo_elemento(&self, p: Punto2) -> Elemento {
        let reservados = || {
            let mut v = Vec::with_capacity(PUNTOS_RESERVADOS);
            v.push(p);
            v
        };
        let figura = match self.herramienta {
            Herramienta::Lapiz => Figura::Lapiz {
                puntos: reservados(),
                // Reservadas igual que los puntos: la primera presion real no
                // puede pedir memoria en el camino caliente.
                presiones: Vec::with_capacity(PUNTOS_RESERVADOS),
                opciones: Some(crate::tinta::OpcionesTinta {
                    variabilidad: self.variabilidad,
                    streamline: crate::tinta::STREAMLINE_RATON_NUEVO,
                }),
            },
            Herramienta::Resaltador => Figura::Resaltador {
                puntos: reservados(),
            },
            Herramienta::Linea => Figura::Linea { puntos: vec![p, p] },
            Herramienta::Flecha => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: false,
                punta_fin: true,
            },
            Herramienta::Elipse => Figura::Elipse,
            Herramienta::Rombo => Figura::Rombo,
            Herramienta::Mosaico => Figura::Mosaico { desenfoque: false },
            // Por dentro es una flecha, como en el movil: asi se edita, se
            // exporta y viaja por el mismo camino que la recta.
            Herramienta::FlechaLibre => Figura::Flecha {
                puntos: reservados(),
                punta_inicio: false,
                punta_fin: true,
            },
            // La de codos nace recta hasta que el grupo A porte `Elbow.kt`.
            // Es una flecha de verdad, no un hueco: llega al movil como
            // flecha y alli se ve; lo que falta es que doble.
            Herramienta::FlechaCodos => Figura::Flecha {
                puntos: vec![p, p],
                punta_inicio: false,
                punta_fin: true,
            },
            // Nace como la GUIA —el ovalo sin repasar—, que es el primer
            // estado de verdad del arco y no un arco a medio hacer.
            Herramienta::Arco => Figura::Arco {
                inicio: 0.0,
                barrido: None,
            },
            // El numero lo pone quien sabe cuantos hay en la escena (el
            // movil suma uno al mayor); el motor no cuenta elementos.
            Herramienta::Serie => Figura::Serie { numero: 1 },
            // La letra la pone la serie de quien la coloque, por lo mismo.
            Herramienta::Punto => Figura::Punto {
                letra: String::new(),
                angulo: -std::f32::consts::FRAC_PI_4,
                radio: 14.0,
            },
            Herramienta::Foco => Figura::Foco { elipse: false },
            Herramienta::Cota => Figura::Cota {
                puntos: reservados(),
            },
            Herramienta::EscalaGrafica => Figura::EscalaGrafica,
            Herramienta::Marco => Figura::Marco {
                // Sin nombre: ponerle uno automatico obligaria a contar los
                // marcos aqui, y el motor no sabe de nombres bonitos.
                nombre: String::new(),
            },
            // Rectangulo y todo lo demas que deje rastro.
            _ => Figura::Rectangulo,
        };
        Elemento {
            id: 0, // lo pone `Escena::anadir`
            figura,
            x: p.x,
            y: p.y,
            ancho: 0.0,
            alto: 0.0,
            angulo: 0.0,
            trazo: self.estilo.trazo,
            // El fondo solo tiene sentido en lo que encierra un area.
            estilo_relleno: self.estilo.estilo_relleno,
            relleno: match self.herramienta {
                Herramienta::Rectangulo
                | Herramienta::Elipse
                | Herramienta::Rombo
                | Herramienta::Arco
                | Herramienta::Serie => self.estilo.relleno,
                _ => None,
            },
            grosor: if self.herramienta == Herramienta::Lapiz {
                self.grosor_tinta
            } else {
                self.estilo.grosor.de_forma()
            },
            estilo: self.estilo.estilo,
            rugosidad: self.estilo.rugosidad,
            opacidad: self.estilo.opacidad,
            semilla: 1,
            version: 0,
            borrado: false,
            grupos: Vec::new(),
            bloqueado: false,
            enlace: None,
            redondo: false,
            // **El trazo nuevo nace con el material elegido**, igual que
            // nace con el color y con el grosor: el panel deja el «actual» y
            // lo siguiente que se dibuje sale de ahi.
            material: self.estilo.material,
            extras: Default::default(),
        }
    }

    /// Que cursor toca en este punto, estando en reposo.
    fn cursor_en(&self, p: Punto2, escena: &Escena, escala: f32) -> FormaCursor {
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                // Van los dos: cual, para saber a que lado apunta; y el
                // angulo, para girar esa direccion con el elemento.
                Some(Agarre::Tamano(t)) => {
                    return FormaCursor::Escalar {
                        tirador: t,
                        angulo: ts.angulo,
                    };
                }
                Some(Agarre::Giro) => return FormaCursor::Giro,
                None => {}
            }
        }
        match self.herramienta {
            Herramienta::Mano => {
                if elemento_en(&escena.elementos, p).is_some() {
                    FormaCursor::Mover
                } else {
                    FormaCursor::Flecha
                }
            }
            Herramienta::Texto => FormaCursor::Texto,
            _ => FormaCursor::Cruz,
        }
    }

    fn pulsar(
        &mut self,
        p: Punto2,
        shift: bool,
        _alt: bool,
        presion: Option<f32>,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        // Un segundo pulsar sin su soltar: la ventana puede recibirlo si el
        // usuario solto fuera. Se ignora en vez de perder el gesto entero.
        if !self.en_reposo() {
            return Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
                pide: None,
            };
        }
        escena.abrir_paso();
        // El punto llega crudo y todas las cribas de abajo lo usan crudo.
        // La pista de un gesto anterior no puede sobrevivir a este clic.
        self.anclaje_activo = None;

        // Elegir, agarrar tiradores y mover es SOLO de la mano (el selector).
        // Con el lapiz en la mano, pulsar encima de un trazo tiene que
        // dibujar, no arrastrarlo: el usuario lo reporto dibujando sobre lo
        // que acababa de trazar y llevandoselo por delante.
        let selecciona = self.herramienta == Herramienta::Mano;

        // 1. Un tirador manda sobre lo que haya debajo. Este SI vale con
        // cualquier herramienta: si se ve el tirador, tiene que agarrar.
        if let Some(ts) = self.tiradores(escena, escala) {
            match ts.en(p, escala) {
                Some(Agarre::Tamano(t)) => {
                    self.estado = Estado::Escalando { tirador: t };
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Escalar {
                            tirador: t,
                            angulo: ts.angulo,
                        },
                        pide: None,
                    };
                }
                Some(Agarre::Giro) => {
                    self.estado = Estado::Girando {
                        anterior: angulo_hacia(ts.centro, p),
                    };
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                        pide: None,
                    };
                }
                None => {}
            }
        }

        // 2. Lo ya seleccionado manda sobre lo de encima.
        let sobre_lo_elegido = self
            .seleccion
            .ids()
            .iter()
            .filter_map(|id| escena.buscar(*id))
            .any(|e| crate::impacto::toca(e, p));
        if selecciona && sobre_lo_elegido && !shift {
            // La instantanea de cada elemento se toma aqui, al pulsar, y no
            // en el primer `mover`: eso deja el camino caliente del
            // arrastre —los avisos del raton que siguen— en cero
            // asignaciones. `apuntar_edicion` ya evita duplicarla si el
            // gesto la vuelve a pedir.
            for &id in self.seleccion.ids() {
                escena.apuntar_edicion(id);
            }
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }

        // 3. Lo que haya bajo el cursor.
        if selecciona && let Some(id) = elemento_en(&escena.elementos, p) {
            if shift {
                self.seleccion.alternar(id);
            } else {
                self.seleccion.poner(id);
            }
            self.estado = Estado::Moviendo { anterior: p };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Mover,
                pide: None,
            };
        }

        // 4. Vacio.
        if self.herramienta == Herramienta::Mano {
            if !shift {
                self.seleccion.limpiar();
            }
            self.estado = Estado::Marquesina {
                origen: p,
                hasta: p,
            };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Flecha,
                pide: None,
            };
        }
        // Escalar no deja rastro: su raya no es un elemento de la escena,
        // solo puntos en `trazo`. Al soltar se convierte en una peticion,
        // no en un `Cambio::Anadido`.
        if self.herramienta == Herramienta::Escalar {
            // Aqui si: ya no hay ninguna criba que envenenar, y este es el
            // enganche que justifica la fase -picar el extremo exacto de
            // una pared de medida conocida-.
            let p = self.enganchar(p, escena, escala);
            self.trazo.clear();
            self.trazo.push(p);
            self.estado = Estado::Calibrando;
            return Respuesta {
                region: Region::Nada,
                cursor: FormaCursor::Cruz,
                pide: None,
            };
        }
        // Con la herramienta de texto, pulsar abre uno para escribir: el que
        // haya bajo el cursor si es un texto, y si no uno nuevo ahi mismo.
        if self.herramienta == Herramienta::Texto {
            self.cerrar_texto(escena);
            let elementos: Vec<Elemento> = escena.visibles().cloned().collect();
            let debajo = elemento_en(&elementos, p).and_then(|id| {
                escena.buscar(id).and_then(|e| match &e.figura {
                    Figura::Texto { texto, .. } => Some((id, texto.clone())),
                    _ => None,
                })
            });
            let (id, contenido) = match debajo {
                Some(par) => par,
                None => {
                    // Nace vacio. Si al terminar sigue vacio, `cerrar_texto`
                    // lo quita: un elemento invisible solo estorba al elegir.
                    let mut e = self.nuevo_elemento(p);
                    e.figura = Figura::Texto {
                        texto: String::new(),
                        tam: crate::texto::TAM_POR_DEFECTO,
                        familia: crate::texto::FAMILIA_POR_DEFECTO.to_string(),
                    };
                    e.x = p.x;
                    e.y = p.y;
                    escena.abrir_paso();
                    let id = escena.anadir(e);
                    escena.cerrar_paso();
                    (id, String::new())
                }
            };
            self.seleccion.poner(id);
            self.escribiendo = Some((id, crate::texto::EdicionTexto::nueva(contenido)));
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Texto,
                pide: None,
            };
        }
        // Texto y emoji no nacen aqui: el texto se escribe, y el emoji lo
        // coloca la sesion del universo con el caracter ya elegido.
        if self.herramienta.deja_rastro()
            && !matches!(self.herramienta, Herramienta::Texto | Herramienta::Emoji)
        {
            // Igual que en calibrar: la figura nace ya pegada al vertice
            // ajeno, pero la decision de que NACE se tomo con el punto
            // crudo.
            let p = self.enganchar(p, escena, escala);
            self.seleccion.limpiar();
            self.trazo.clear();
            self.trazo.push(p);
            let mut e = self.nuevo_elemento(p);
            // Con lapiz, la muestra de presion puede llegar en el mismo
            // mensaje que el clic. Sin raton no hay presion: el trazo
            // arranca sin ella y `anadir_a_lapiz` la rellena hacia atras en
            // cuanto llegue la primera de verdad.
            if let (
                Some(pr),
                Figura::Lapiz {
                    presiones,
                    opciones,
                    ..
                },
            ) = (presion, &mut e.figura)
            {
                presiones.push(pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
            let id = escena.anadir(e);
            self.forma_elipse = None;
            self.estado = Estado::Dibujando { id };
            return Respuesta {
                region: Region::Todo,
                cursor: FormaCursor::Cruz,
                pide: None,
            };
        }
        Respuesta {
            region: Region::Nada,
            cursor: FormaCursor::Cruz,
            pide: None,
        }
    }

    fn mover(
        &mut self,
        p: Punto2,
        shift: bool,
        alt: bool,
        presion: Option<f32>,
        escena: &mut Escena,
        escala: f32,
    ) -> Respuesta {
        match self.estado {
            Estado::Reposo => Respuesta {
                region: Region::Nada,
                cursor: self.cursor_en(p, escena, escala),
                pide: None,
            },

            Estado::Dibujando { id } => {
                // El camino caliente. Ni una asignacion: el buffer ya esta
                // reservado, y se ensucia el tramo y no la pantalla.
                let anterior = *self.trazo.last().unwrap_or(&p);
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                }
                let grosor = escena.buscar(id).map_or(3.0, |e| e.grosor);
                // Lapiz y Resaltador pintan mucho mas ancho que su `grosor`
                // (el contorno de `freehand` llega a `grosor * FACTOR_VARIABLE`
                // para el Lapiz y a `grosor * 3.0` para el Resaltador), y el
                // streamline atrasa los puntos ya suavizados: la caja
                // anterior->p con margen `grosor/2` dejaba fuera casi todo el
                // contorno que de verdad cambia, y el trazo dejaba estela.
                //
                // La caja se calcula sobre los puntos de la FIGURA
                // (`puntos`, ya sin duplicados exactos), no sobre
                // `self.trazo`: ese buffer deja de crecer a partir de
                // `PUNTOS_RESERVADOS` (512) para no asignar, asi que un
                // trazo mas largo lo dejaria congelado en un tramo viejo y
                // la tinta nueva quedaria fuera de la zona presentada.
                let mut caja_ancha: Option<(f32, f32, f32, f32)> = None;
                // Las figuras (no el trazo) cambian ENTERAS al mover el cursor: un
                // rectangulo crece desde donde se pulso y sus lados lejanos
                // tambien se mueven. La zona sucia es su caja antes y despues;
                // con solo el tramo anterior->p esos lados se presentaban tarde
                // y la figura parecia arrastrarse (lo noto el usuario).
                let caja_antes = escena.buscar(id).map(|e| e.caja());
                let elipse_rapida = self.forma_elipse;
                if let Some(e) = escena.buscar_mut(id) {
                    match &mut e.figura {
                        Figura::Lapiz {
                            puntos,
                            presiones,
                            opciones,
                        } => {
                            anadir_a_lapiz(puntos, presiones, opciones, p, presion);
                            let margen = grosor * crate::tinta::FACTOR_VARIABLE + 2.0;
                            caja_ancha = Some(caja_ancha_de(puntos, p, margen));
                        }
                        Figura::Resaltador { puntos } => {
                            if puntos.last() != Some(&p) {
                                puntos.push(p);
                            }
                            let margen = grosor * 3.0 + 2.0;
                            caja_ancha = Some(caja_ancha_de(puntos, p, margen));
                        }
                        Figura::Linea { puntos } | Figura::Flecha { puntos, .. } => {
                            // Linea y flecha son dos puntos: el segundo sigue
                            // al cursor en vez de acumularse.
                            if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        Figura::Cota { puntos } => {
                            // Como la Linea: el segundo punto sigue al
                            // cursor. Pero `nuevo_elemento` la arranca con
                            // un solo punto (`reservados()`), asi que el
                            // primer aviso anade el segundo en vez de
                            // sustituirlo.
                            if puntos.len() < 2 {
                                puntos.push(p);
                            } else if let Some(ultimo) = puntos.last_mut() {
                                *ultimo = p;
                            }
                        }
                        Figura::Elipse if elipse_rapida.is_some() => {
                            // Forma rapida: escala desde el centro, en
                            // proporcion a como se aleja el cursor.
                            if let Some((c, d0, rx, ry)) = elipse_rapida {
                                let k = p.distancia(c) / d0.max(1.0);
                                e.x = c.x - rx * k;
                                e.y = c.y - ry * k;
                                e.ancho = 2.0 * rx * k;
                                e.alto = 2.0 * ry * k;
                            }
                        }
                        _ => {
                            // Las figuras de caja crecen desde donde se
                            // pulso. La escala grafica cae aqui tambien.
                            let o = *self.trazo.first().unwrap_or(&p);
                            e.x = o.x.min(p.x);
                            e.y = o.y.min(p.y);
                            e.ancho = (p.x - o.x).abs();
                            e.alto = (p.y - o.y).abs();
                        }
                    }
                    e.tocar();
                }
                if caja_ancha.is_none() {
                    if let (Some(a), Some(e)) = (caja_antes, escena.buscar(id)) {
                        let d = e.caja();
                        // Margen para el grosor, el temblor de la rugosidad y la
                        // punta de una flecha, que salen de la caja geometrica.
                        let m = e.grosor * 2.0 + 4.0 * e.rugosidad + 24.0;
                        caja_ancha = Some((
                            a.0.min(d.0) - m,
                            a.1.min(d.1) - m,
                            a.2.max(d.2) + m,
                            a.3.max(d.3) + m,
                        ));
                    }
                }
                let region = match caja_ancha {
                    // El contorno de un punto nuevo se apoya en varios
                    // puntos anteriores (streamline y suavizado), y el
                    // extremo del trazo se recalcula entero: la caja tiene
                    // que cubrir la cola de puntos de la figura, no solo el
                    // tramo anterior->p.
                    Some((x0, y0, x1, y1)) => Region::Caja(x0, y0, x1, y1),
                    None => {
                        let m = grosor / 2.0 + 1.0;
                        Region::Caja(
                            anterior.x.min(p.x) - m,
                            anterior.y.min(p.y) - m,
                            anterior.x.max(p.x) + m,
                            anterior.y.max(p.y) + m,
                        )
                    }
                };
                Respuesta {
                    region,
                    cursor: FormaCursor::Cruz,
                    pide: None,
                }
            }

            Estado::Moviendo { anterior } => {
                let (dx, dy) = (p.x - anterior.x, p.y - anterior.y);
                // Un marco arrastra lo que encierra (fase 5): es para lo que
                // sirve. Se pregunta primero si hay alguno porque lo normal
                // es que no: `con_contenidos` reserva memoria, y esto corre en
                // cada movimiento del raton (prueba `asignaciones`).
                let hay_marco = self
                    .seleccion
                    .ids()
                    .iter()
                    .any(|id| escena.buscar(*id).is_some_and(crate::marco::es_marco));
                let ids: Vec<u64> = if hay_marco {
                    crate::marco::con_contenidos(&escena.elementos, self.seleccion.ids())
                } else {
                    Vec::new()
                };
                let sueltos;
                let ids: &[u64] = if hay_marco {
                    &ids
                } else {
                    sueltos = self.seleccion.ids();
                    sueltos
                };
                for &id in ids {
                    // Sin esto el paso queda vacio y no hay nada que
                    // deshacer. Es el error mas facil de cometer aqui.
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        e.mover(dx, dy);
                    }
                }
                self.estado = Estado::Moviendo { anterior: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Mover,
                    pide: None,
                }
            }

            Estado::Escalando { tirador } => {
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::escalar(e, tirador, p, shift, alt);
                    }
                }
                let angulo = self.tiradores(escena, escala).map_or(0.0, |t| t.angulo);
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Escalar { tirador, angulo },
                    pide: None,
                }
            }

            Estado::Girando { anterior } => {
                let Some(centro) = self.seleccion.centro(escena) else {
                    return Respuesta {
                        region: Region::Nada,
                        cursor: FormaCursor::Giro,
                        pide: None,
                    };
                };
                let ahora = angulo_hacia(centro, p);
                let ahora = if shift { a_saltos(ahora) } else { ahora };
                let delta = ahora - anterior;
                for &id in self.seleccion.ids() {
                    escena.apuntar_edicion(id);
                    if let Some(e) = escena.buscar_mut(id) {
                        transformar::girar(e, centro, delta);
                    }
                }
                self.estado = Estado::Girando { anterior: ahora };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Giro,
                    pide: None,
                }
            }

            Estado::Marquesina { origen, .. } => {
                self.estado = Estado::Marquesina { origen, hasta: p };
                Respuesta {
                    region: Region::Todo,
                    cursor: FormaCursor::Flecha,
                    pide: None,
                }
            }

            Estado::Calibrando => {
                // Calibrar sobre el borde de una pared es el caso normal, y
                // a pulso sale torcida: un grado de mas son dos centimetros
                // de error por metro (diseno S5). Con Shift se sujeta a
                // horizontal o vertical, con el mismo mecanismo que ya usan
                // Linea y Flecha en `pixpin-ui/anotador.rs` -aqui con 2
                // pasos por media vuelta en vez de 12, porque calibrar solo
                // ofrece las dos, no las diagonales.
                let origen = *self.trazo.first().unwrap_or(&p);
                let p = if shift {
                    restringir_angulo(origen, p, 2.0)
                } else {
                    p
                };
                // Mismo buffer que `Dibujando`, ya reservado: mover el raton
                // calibrando tampoco asigna memoria.
                if self.trazo.len() < PUNTOS_RESERVADOS {
                    self.trazo.push(p);
                } else if let Some(ultimo) = self.trazo.last_mut() {
                    *ultimo = p;
                }
                Respuesta {
                    region: Region::Nada,
                    cursor: FormaCursor::Cruz,
                    pide: None,
                }
            }
        }
    }

    fn soltar(&mut self, p: Punto2, escena: &mut Escena, escala: f32) -> Respuesta {
        if let Estado::Marquesina { origen, .. } = self.estado {
            let caja = (origen.x, origen.y, p.x, p.y);
            let cogidos = dentro_de(&escena.elementos, caja);
            self.seleccion.poner_todos(cogidos);
        }
        // Escalar no deja rastro: al soltar, la distancia entre el primer y
        // el ultimo punto se convierte en una peticion. Menos de dos
        // pixeles DE PANTALLA es un clic, no una medida, y no se pide nada.
        // `largo_px` viaja en unidades de MUNDO -son las de `p`-, asi que el
        // umbral tiene que pasar por `escala` (unidades de mundo por pixel
        // de pantalla, la misma convencion que `HOLGURA_SELECCION*escala`
        // en `pintado::marco_de_seleccion`) o dejaria de ser dos pixeles en
        // cuanto la camara tuviera zoom.
        let pide = if let Estado::Calibrando = self.estado {
            let origen = *self.trazo.first().unwrap_or(&p);
            // El extremo es el ultimo punto del `trazo`, no el `p` crudo del
            // evento: es el que `mover` dejo ya sujeto a Shift. Si se usara
            // `p` a pulso, la sujecion de arriba pintaria la raya recta
            // mientras se arrastra y luego mediria la torcida real al
            // soltar -justo el fallo que Shift existe para evitar.
            let fin = self.trazo.last().copied().unwrap_or(p);
            let largo_px = origen.distancia(fin);
            (largo_px >= 2.0 * escala.max(f32::EPSILON)).then_some(Peticion::Calibrar { largo_px })
        } else {
            None
        };
        // Un paso sin cambios no entra en el historial, asi que hacer clic
        // sin arrastrar no consume un Ctrl+Z. De eso se encarga cerrar_paso.
        escena.cerrar_paso();
        self.forma_elipse = None;
        self.estado = Estado::Reposo;
        Respuesta {
            region: Region::Todo,
            cursor: FormaCursor::Flecha,
            pide,
        }
    }
}

/// Hacia donde apunta un tirador, en un elemento girado `angulo`.
///
/// Cero es hacia arriba y crece en el sentido de las agujas, igual que
/// `transformar::angulo_hacia`. La ventana usa esto para elegir entre las
/// cuatro flechas que trae Windows.
pub fn direccion_del_tirador(t: Tirador, angulo: f32) -> f32 {
    use std::f32::consts::PI;
    let base = match t {
        Tirador::NorteBorde => 0.0,
        Tirador::NoresteEsquina => PI / 4.0,
        Tirador::EsteBorde => PI / 2.0,
        Tirador::SuresteEsquina => 3.0 * PI / 4.0,
        Tirador::SurBorde => PI,
        Tirador::SuroesteEsquina => 5.0 * PI / 4.0,
        Tirador::OesteBorde => 3.0 * PI / 2.0,
        Tirador::NoroesteEsquina => 7.0 * PI / 4.0,
    };
    base + angulo
}

/// La caja que envuelve los ultimos `n` puntos de una cola de puntos (o
/// todos si hay menos). Sin asignar: solo recorre la cola de un slice ya
/// reservado -es el camino caliente de `mover` mientras se dibuja a mano-.
fn caja_de_cola(puntos: &[Punto2], n: usize) -> (f32, f32, f32, f32) {
    let inicio = puntos.len().saturating_sub(n);
    let cola = &puntos[inicio..];
    let mut caja = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for p in cola {
        caja.0 = caja.0.min(p.x);
        caja.1 = caja.1.min(p.y);
        caja.2 = caja.2.max(p.x);
        caja.3 = caja.3.max(p.y);
    }
    if cola.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        caja
    }
}

/// La caja sucia de Lapiz/Resaltador mientras se dibuja: la cola de los
/// ultimos 6 puntos DE LA FIGURA (no de `self.trazo`, que deja de crecer a
/// los 512 puntos para no asignar y por eso no vale como referencia de un
/// trazo largo), con el punto nuevo `p` metido en el minimo/maximo por si
/// se dedujo como duplicado exacto y no llego a entrar en `puntos`, y
/// expandida por `margen` (el radio maximo que puede alcanzar el contorno
/// de `freehand` para ese grosor).
fn caja_ancha_de(puntos: &[Punto2], p: Punto2, margen: f32) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = caja_de_cola(puntos, 6);
    x0 = x0.min(p.x);
    y0 = y0.min(p.y);
    x1 = x1.max(p.x);
    y1 = y1.max(p.y);
    (x0 - margen, y0 - margen, x1 + margen, y1 + margen)
}

/// Anade un punto a un trazo de lapiz respetando sus presiones.
///
/// - Solo se descartan duplicados exactos (D109), como Excalidraw.
/// - La primera presion real rellena hacia atras los puntos que llegaron sin
///   ella (el clic lo entrega un mensaje de raton, la presion llega despues)
///   y cambia el suavizado al del lapiz.
/// - Un punto sin presion en un trazo que ya la tiene repite la ultima, para
///   que las dos listas no se desalineen.
fn anadir_a_lapiz(
    puntos: &mut Vec<Punto2>,
    presiones: &mut Vec<f32>,
    opciones: &mut Option<crate::tinta::OpcionesTinta>,
    p: Punto2,
    presion: Option<f32>,
) {
    if puntos.last() == Some(&p) {
        return;
    }
    puntos.push(p);
    match presion {
        Some(pr) => {
            if presiones.is_empty() {
                presiones.resize(puntos.len() - 1, pr);
                if let Some(o) = opciones {
                    o.streamline = crate::tinta::STREAMLINE_LAPIZ;
                }
            }
            presiones.push(pr);
        }
        None => {
            if let Some(&ultima) = presiones.last() {
                presiones.push(ultima);
            }
        }
    }
}

impl Gesto {
    /// Mete una letra en el texto que se esta escribiendo. Devuelve si algo
    /// cambio, para que la ventana sepa si repintar.
    pub fn escribir(&mut self, c: char, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.as_mut() else {
            return false;
        };
        edicion.insertar(c);
        let texto = edicion.texto().to_string();
        Self::volcar_texto(escena, *id, &texto);
        true
    }

    /// Una tecla de edicion (flechas, borrar, entrar).
    pub fn tecla_de_texto(&mut self, t: crate::texto::TeclaTexto, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.as_mut() else {
            return false;
        };
        if !edicion.tecla(t) {
            return false;
        }
        let texto = edicion.texto().to_string();
        Self::volcar_texto(escena, *id, &texto);
        true
    }

    /// Termina de escribir.
    ///
    /// Un texto que quedo vacio se borra: dejarlo seria un elemento
    /// invisible que se puede elegir sin querer y que nadie sabria quitar.
    pub fn cerrar_texto(&mut self, escena: &mut Escena) -> bool {
        let Some((id, edicion)) = self.escribiendo.take() else {
            return false;
        };
        if edicion.esta_vacio() {
            escena.abrir_paso();
            escena.apuntar_edicion(id);
            if let Some(e) = escena.buscar_mut(id) {
                e.borrado = true;
                e.tocar();
            }
            escena.cerrar_paso();
            self.seleccion.limpiar();
        }
        true
    }

    /// Si se esta escribiendo ahora mismo. La ventana lo mira para saber si
    /// una tecla es texto o un atajo de herramienta.
    pub fn esta_escribiendo(&self) -> bool {
        self.escribiendo.is_some()
    }

    /// Lo escrito y donde va el cursor, para pintarlo.
    pub fn texto_en_curso(&self) -> Option<(u64, &crate::texto::EdicionTexto)> {
        self.escribiendo.as_ref().map(|(id, e)| (*id, e))
    }

    /// Escribe el texto en su elemento y le ajusta la caja.
    ///
    /// Todo dentro de UN paso de deshacer por pulsacion no vale: serian
    /// treinta pasos para una palabra. Se apunta la edicion una vez, al
    /// abrirse el texto, y aqui solo se cambia el contenido.
    fn volcar_texto(escena: &mut Escena, id: u64, texto: &str) {
        let Some(e) = escena.buscar_mut(id) else {
            return;
        };
        let tam = match &e.figura {
            Figura::Texto { tam, .. } => *tam,
            _ => crate::texto::TAM_POR_DEFECTO,
        };
        if let Figura::Texto { texto: dentro, .. } = &mut e.figura {
            dentro.clear();
            dentro.push_str(texto);
        }
        let (ancho, alto) = crate::texto::medida_estimada(texto, tam);
        e.ancho = ancho;
        e.alto = alto;
        e.tocar();
    }
}
#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::elemento::{ColorRgba, EstiloTrazo, Figura};
    use pixpin_geom::Tirador;

    fn rect(x: f32, y: f32, ancho: f32, alto: f32) -> Elemento {
        Elemento {
            id: 0,
            figura: Figura::Rectangulo,
            x,
            y,
            ancho,
            alto,
            angulo: 0.0,
            trazo: ColorRgba::opaco(0.0, 0.0, 0.0),
            estilo_relleno: Default::default(),
            relleno: Some(ColorRgba::opaco(1.0, 0.0, 0.0)),
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

    fn pulsar(p: Punto2) -> EventoGesto {
        EventoGesto::Pulsar {
            p,
            shift: false,
            alt: false,
            presion: None,
        }
    }
    fn mover(p: Punto2) -> EventoGesto {
        EventoGesto::Mover {
            p,
            shift: false,
            alt: false,
            presion: None,
        }
    }

    #[test]
    fn con_el_lapiz_pulsar_encima_de_un_trazo_dibuja_y_no_lo_mueve() {
        // El fallo que reporto el usuario: dibujaba encima de lo que acababa
        // de trazar y se lo llevaba por delante en vez de seguir dibujando.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let antes = escena.buscar(id).unwrap().clone();

        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(80.0, 80.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(80.0, 80.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(
            escena.buscar(id).unwrap().x,
            antes.x,
            "el rectangulo se movio"
        );
        assert_eq!(escena.elementos.len(), 2, "y el trazo nuevo no se hizo");
        assert_eq!(g.seleccion.cuantos(), 0, "ni se eligio nada");
    }

    /// Igual que `pulsar`/`mover`, pero llevando la presion del lapiz.
    fn pulsar_con_gesto(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(x, y),
                shift: false,
                alt: false,
                presion,
            },
            e,
            1.0,
        );
    }
    fn mover_con_gesto(g: &mut Gesto, e: &mut Escena, x: f32, y: f32, presion: Option<f32>) {
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(x, y),
                shift: false,
                alt: false,
                presion,
            },
            e,
            1.0,
        );
    }
    fn lapiz_de(
        e: &Escena,
    ) -> (
        Vec<Punto2>,
        Vec<f32>,
        Option<crate::tinta::OpcionesTinta>,
        f32,
    ) {
        let el = e.visibles().last().expect("hay un trazo");
        let Figura::Lapiz {
            puntos,
            presiones,
            opciones,
        } = &el.figura
        else {
            panic!("no es lapiz")
        };
        (puntos.clone(), presiones.clone(), *opciones, el.grosor)
    }

    #[test]
    fn con_raton_el_trazo_nace_con_la_pluma_elegida_y_sin_presiones() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.variabilidad = crate::tinta::Variabilidad::Constante;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        let (_, presiones, opciones, grosor) = lapiz_de(&e);
        assert!(presiones.is_empty());
        assert_eq!(grosor, crate::tinta::GROSOR_GRUESO);
        let o = opciones.expect("un trazo nuevo nunca es legado");
        assert_eq!(o.variabilidad, crate::tinta::Variabilidad::Constante);
        assert_eq!(o.streamline, crate::tinta::STREAMLINE_RATON_NUEVO);
    }

    #[test]
    fn la_primera_presion_real_rellena_hacia_atras_y_afina_el_suavizado() {
        // El boton lo pulsa un mensaje de raton sin presion; la presion
        // llega con la primera muestra del lapiz.
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, Some(0.7));
        mover_con_gesto(&mut g, &mut e, 9.0, 0.0, Some(0.9));
        let (puntos, presiones, opciones, _) = lapiz_de(&e);
        assert_eq!(puntos.len(), presiones.len());
        assert_eq!(presiones, vec![0.7, 0.7, 0.9]);
        assert_eq!(opciones.unwrap().streamline, crate::tinta::STREAMLINE_LAPIZ);
    }

    /// El hallazgo critico de la revision de la Tarea 9: `Region::Caja`
    /// solo cubria el tramo anterior->p con margen `grosor/2`, pero el
    /// contorno de `freehand` es mucho mas ancho que eso (`grosor *
    /// FACTOR_VARIABLE`) y el streamline mueve puntos YA pintados varias
    /// muestras atras -sin contar que el remate del trazo se recalcula
    /// entero en cada punto nuevo-. Con la caja vieja, dibujar a mano
    /// dejaba estela en pantalla.
    ///
    /// La prueba compara el contorno con diecinueve puntos contra el de
    /// veinte y comprueba que CUALQUIER punto que no tenga pareja casi
    /// exacta en el otro contorno -es decir, cualquier punto que de verdad
    /// cambio de sitio entre un fotograma y el siguiente- cae dentro de la
    /// `Region::Caja` que devuelve el ultimo `mover`.
    ///
    /// No se uso la receta literal de la revision ("los ultimos ~40 puntos
    /// del contorno viejo son su remate"): en `tinta::freehand::contorno`,
    /// el orden real del `Vec` que se devuelve es `izquierda ++ tapa_fin ++
    /// derecha.rev() ++ tapa_inicio` (ver `freehand.rs`), asi que los
    /// ULTIMOS puntos del array son `tapa_inicio` -la tapa del PRINCIPIO
    /// del trazo, junto al primer clic-, no el remate del extremo que
    /// crece. Esos puntos no cambian de un fotograma a otro (dependen solo
    /// de los primeros puntos en bruto, que no se tocan al anadir uno
    /// nuevo al final), así que exigir que caigan en la caja sucia habria
    /// forzado una caja mas grande de lo necesario sin detectar ningun
    /// pixel que de verdad cambie. La comparacion punto a punto contra el
    /// OTRO contorno mide lo que realmente hay que repintar, sin asumir en
    /// que posicion del array cae el remate.
    #[test]
    fn la_region_sucia_de_dibujar_a_mano_cubre_todo_lo_que_cambia_en_el_contorno() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;

        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        // 18 movimientos mas: 19 puntos en total antes del ultimo, un
        // zigzag para que el contorno de verdad cambie de forma con cada
        // punto nuevo (una recta apenas mueve nada al alargarse).
        for i in 1..19 {
            let x = i as f32 * 3.0;
            let y = if i % 2 == 0 { 0.0 } else { 6.0 };
            mover_con_gesto(&mut g, &mut e, x, y, None);
        }
        let (puntos_antes, presiones_antes, opciones, grosor) = lapiz_de(&e);
        assert_eq!(puntos_antes.len(), 19);

        // El punto 20, con su Respuesta de verdad (la que se comprueba).
        let r = g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(19.0 * 3.0, 6.0),
                shift: false,
                alt: false,
                presion: None,
            },
            &mut e,
            1.0,
        );
        let Region::Caja(cx0, cy0, cx1, cy1) = r.region else {
            panic!("dibujar a mano tiene que ensuciar una caja")
        };

        let (puntos_despues, presiones_despues, _, _) = lapiz_de(&e);
        assert_eq!(puntos_despues.len(), 20);

        let contorno_antes =
            crate::tinta::contorno_de_lapiz(&puntos_antes, &presiones_antes, grosor, opciones);
        let contorno_despues =
            crate::tinta::contorno_de_lapiz(&puntos_despues, &presiones_despues, grosor, opciones);

        let dentro = |x: f32, y: f32| x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1;
        // Casi exacta: la misma entrada por el mismo camino de codigo da el
        // mismo `f64` bit a bit, pero un margen pequeno cubre por si algun
        // punto se reordena sin desplazarse de verdad.
        let tiene_pareja = |p: Punto2, otro: &[Punto2]| {
            otro.iter()
                .any(|q| (p.x - q.x).abs() < 0.01 && (p.y - q.y).abs() < 0.01)
        };

        // Todo punto que aparece en un contorno y no en el otro es, por
        // definicion, un pixel que cambio entre los dos fotogramas.
        let mut algo_cambio = false;
        for p in &contorno_despues {
            if !tiene_pareja(*p, &contorno_antes) {
                algo_cambio = true;
                assert!(
                    dentro(p.x, p.y),
                    "el contorno nuevo en ({}, {}) no estaba antes y cae fuera de la region sucia",
                    p.x,
                    p.y
                );
            }
        }
        for p in &contorno_antes {
            if !tiene_pareja(*p, &contorno_despues) {
                algo_cambio = true;
                assert!(
                    dentro(p.x, p.y),
                    "el contorno viejo en ({}, {}) desaparece y cae fuera de la region sucia",
                    p.x,
                    p.y
                );
            }
        }
        assert!(
            algo_cambio,
            "el trazo de prueba tiene que hacer cambiar el contorno, o esto no mide nada"
        );
    }

    /// La regresion de la ronda 2: `self.trazo` deja de crecer a partir de
    /// `PUNTOS_RESERVADOS` (512 puntos) para no asignar, pero la caja ancha
    /// se calculaba sobre `self.trazo` -asi que un trazo mas largo que eso
    /// se quedaba congelado en el tramo 507-512 y la Region::Caja nunca
    /// volvia a contener el punto nuevo `p`. Con la capa congelada valida,
    /// esa tinta quedaba invisible hasta el siguiente present completo (y
    /// con la Tarea 7 recuperando cada punto fusionado, 512 movimientos son
    /// entre medio segundo y varios segundos de trazo real).
    ///
    /// Esta prueba dibuja 600 puntos DISTINTOS (bien separados: nada de
    /// duplicados que el propio `puntos` de la figura deduplique) y
    /// comprueba que la `Region::Caja` del ultimo movimiento contiene el
    /// punto nuevo y los cinco anteriores de la FIGURA (no de `self.trazo`,
    /// que para entonces ya esta congelado).
    #[test]
    fn un_trazo_de_mas_de_quinientos_puntos_sigue_ensuciando_la_punta_que_crece() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        g.grosor_tinta = crate::tinta::GROSOR_GRUESO;

        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        // 599 movimientos mas: 600 puntos en total, todos distintos entre
        // si (avanzan siempre en x) para que ninguno se deduplique.
        for i in 1..600 {
            mover_con_gesto(&mut g, &mut e, i as f32, 0.0, None);
        }
        let (puntos, ..) = lapiz_de(&e);
        assert_eq!(
            puntos.len(),
            600,
            "ninguno de los 600 puntos era un duplicado"
        );

        // El punto 601, con su Respuesta de verdad.
        let ultimo = Punto2::nuevo(600.0, 0.0);
        let r = g.evento(
            EventoGesto::Mover {
                p: ultimo,
                shift: false,
                alt: false,
                presion: None,
            },
            &mut e,
            1.0,
        );
        let Region::Caja(cx0, cy0, cx1, cy1) = r.region else {
            panic!("dibujar a mano tiene que ensuciar una caja")
        };
        let dentro = |x: f32, y: f32| x >= cx0 && x <= cx1 && y >= cy0 && y <= cy1;

        assert!(
            dentro(ultimo.x, ultimo.y),
            "el punto nuevo ({}, {}) tiene que estar dentro de la region sucia",
            ultimo.x,
            ultimo.y
        );

        let (puntos_despues, ..) = lapiz_de(&e);
        assert_eq!(puntos_despues.len(), 601);
        for p in puntos_despues.iter().rev().take(6) {
            assert!(
                dentro(p.x, p.y),
                "el punto de la figura ({}, {}) -de la cola reciente- tiene que \
                 estar dentro de la region sucia",
                p.x,
                p.y
            );
        }
    }

    #[test]
    fn un_punto_repetido_exacto_no_se_guarda() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        mover_con_gesto(&mut g, &mut e, 5.0, 0.0, None);
        assert_eq!(lapiz_de(&e).0.len(), 2);
    }

    #[test]
    fn trazo_en_curso_solo_existe_mientras_se_dibuja_a_mano() {
        let (mut g, mut e) = (Gesto::nuevo(), Escena::nueva());
        g.herramienta = Herramienta::Lapiz;
        assert_eq!(g.trazo_en_curso(), None);
        pulsar_con_gesto(&mut g, &mut e, 0.0, 0.0, None);
        assert!(g.trazo_en_curso().is_some());
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(1.0, 0.0),
            },
            &mut e,
            1.0,
        );
        assert_eq!(g.trazo_en_curso(), None);
    }

    /// Un arrastre entero: pulsar, mover y soltar.
    fn arrastrar(g: &mut Gesto, e: &mut Escena, de: Punto2, a: Punto2) {
        g.evento(pulsar(de), e, 1.0);
        g.evento(mover(a), e, 1.0);
        g.evento(EventoGesto::Soltar { p: a }, e, 1.0);
    }

    #[test]
    fn un_arrastre_entero_deja_exactamente_un_paso_de_deshacer() {
        // El invariante que hace util el historial de la tarea 1.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(50.0, 50.0),
            Punto2::nuevo(150.0, 50.0),
        );
        assert_eq!(escena.buscar(id).unwrap().x, 100.0, "se movio");

        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "de una vez");
    }

    #[test]
    fn escape_a_mitad_de_un_arrastre_lo_cancela() {
        // Sale gratis: el paso ya guarda el estado anterior, asi que
        // cancelar es aplicarlo y tirar el paso.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(500.0, 500.0)), &mut escena, 1.0);
        g.evento(EventoGesto::Escape, &mut escena, 1.0);

        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "volvio a su sitio");
        assert!(g.en_reposo());
    }

    #[test]
    fn un_tirador_manda_sobre_el_elemento_que_haya_debajo() {
        // Sin esta prioridad, un tirador encima de otro elemento es
        // inalcanzable: el clic cae en el de debajo.
        let mut escena = Escena::nueva();
        let grande = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(grande);

        // La esquina sureste del grande, que cae DENTRO del propio grande.
        let ts = Tiradores::de_elemento(escena.buscar(grande).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;

        g.evento(pulsar(se), &mut escena, 1.0);
        g.evento(
            mover(Punto2::nuevo(se.x + 100.0, se.y + 100.0)),
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(se.x + 100.0, se.y + 100.0),
            },
            &mut escena,
            1.0,
        );

        let e = escena.buscar(grande).unwrap();
        assert!(e.ancho > 250.0, "escalo en vez de moverse: {}", e.ancho);
        assert_eq!(e.x, 0.0, "y no se movio");
    }

    #[test]
    fn lo_ya_seleccionado_manda_sobre_lo_que_haya_encima() {
        // Sin esta regla, mover un grupo se convierte en seleccionar por
        // accidente lo que estaba encima.
        let mut escena = Escena::nueva();
        let abajo = escena.anadir(rect(0.0, 0.0, 200.0, 200.0));
        let encima = escena.anadir(rect(50.0, 50.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(abajo);

        // Pulsar donde estan los dos: gana el ya elegido.
        //
        // El destino del arrastre se aleja hasta 175 -y no los 85
        // originales- porque ahora `Estado::Moviendo` es `Faena::Moviendo` a
        // ojos del iman: (85,75) cae a 15 del lado derecho de "encima" y a
        // 10 de su centro, dentro del radio de 21 (14 x 1,5) que usa
        // `Faena::Moviendo`. El punto se pegaba al propio "encima" -que no
        // esta en `excluir` porque no es el seleccionado- y el arrastre se
        // quedaba en delta cero. A 175 no hay ningun ancla de "encima" a
        // menos de 21.
        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(75.0, 75.0),
            Punto2::nuevo(175.0, 75.0),
        );

        assert_eq!(g.seleccion.ids(), &[abajo], "sigue el de antes");
        assert_eq!(
            escena.buscar(abajo).unwrap().x,
            100.0,
            "se movio el de antes"
        );
        assert_eq!(
            escena.buscar(encima).unwrap().x,
            50.0,
            "el de encima, quieto"
        );
    }

    #[test]
    fn arrastrar_en_vacio_con_la_mano_hace_marquesina() {
        let mut escena = Escena::nueva();
        let dentro = escena.anadir(rect(20.0, 20.0, 30.0, 30.0));
        let fuera = escena.anadir(rect(500.0, 500.0, 30.0, 30.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(g.marquesina().is_some(), "hay marquesina en curso");
        g.evento(mover(Punto2::nuevo(200.0, 200.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(200.0, 200.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[dentro]);
        assert!(!g.seleccion.contiene(fuera));
        assert!(g.marquesina().is_none(), "y se acabo al soltar");
    }

    #[test]
    fn shift_mas_clic_anade_a_la_seleccion() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 50.0, 50.0));
        let b = escena.anadir(rect(100.0, 0.0, 50.0, 50.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;

        g.evento(pulsar(Punto2::nuevo(25.0, 25.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(25.0, 25.0),
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Pulsar {
                p: Punto2::nuevo(125.0, 25.0),
                shift: true,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(125.0, 25.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(g.seleccion.ids(), &[a, b]);
    }

    #[test]
    fn dibujar_un_trazo_lo_anade_y_lo_deja_deshacible() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..20 {
            g.evento(mover(Punto2::nuevo(i as f32 * 5.0, 0.0)), &mut escena, 1.0);
        }
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(95.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0, "un trazo, un Ctrl+Z");
    }

    #[test]
    fn dibujando_se_ensucia_el_tramo_y_no_la_pantalla() {
        // En 1080p es la diferencia entre dos millones de pixeles por
        // fotograma y unos cientos.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(10.0, 0.0)), &mut escena, 1.0);

        let Region::Caja(x0, y0, x1, y1) = r.region else {
            panic!("tiene que ser una caja, es {:?}", r.region);
        };
        assert!(x1 - x0 < 40.0 && y1 - y0 < 40.0, "el tramo, no la pantalla");
    }

    #[test]
    fn dibujar_no_asigna_memoria_por_cada_aviso_del_raton() {
        // El buffer del trazo en curso se reserva de una vez y se reutiliza
        // entre trazos con clear(). La prueba de verdad —contar
        // asignaciones— es la tarea 15; aqui se comprueba lo que se puede
        // observar desde dentro.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        for i in 1..400 {
            g.evento(mover(Punto2::nuevo(i as f32, 0.0)), &mut escena, 1.0);
        }
        assert!(
            g.capacidad_del_trazo() >= PUNTOS_RESERVADOS,
            "se reservo de una vez"
        );
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(400.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        let tras_el_primero = g.capacidad_del_trazo();
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert_eq!(
            g.capacidad_del_trazo(),
            tras_el_primero,
            "el segundo trazo reutiliza el buffer del primero"
        );
    }

    #[test]
    fn suprimir_borra_lo_seleccionado_de_una_vez() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        let mut g = Gesto::nuevo();
        g.seleccion.poner_todos([a, b]);

        g.evento(EventoGesto::Suprimir, &mut escena, 1.0);
        assert_eq!(escena.cuantos_visibles(), 0);

        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 2, "los dos vuelven de una vez");
    }

    #[test]
    fn seleccionar_todo_no_coge_los_borrados() {
        let mut escena = Escena::nueva();
        let a = escena.anadir(rect(0.0, 0.0, 10.0, 10.0));
        let b = escena.anadir(rect(20.0, 0.0, 10.0, 10.0));
        escena.borrar(b);
        let mut g = Gesto::nuevo();

        g.evento(EventoGesto::SeleccionarTodo, &mut escena, 1.0);
        assert_eq!(g.seleccion.ids(), &[a]);
    }

    #[test]
    fn pulsar_dos_veces_sin_soltar_no_pierde_el_gesto() {
        // La ventana puede recibir un WM_LBUTTONDOWN sin su WM_LBUTTONUP si
        // el usuario suelta fuera. Perder el gesto entero por eso seria
        // peor que ignorar el segundo pulsar.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(150.0, 50.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(150.0, 50.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.buscar(id).unwrap().x, 100.0);
        assert!(escena.deshacer());
        assert_eq!(escena.buscar(id).unwrap().x, 0.0, "sigue siendo un paso");
    }

    #[test]
    fn un_clic_sobre_lo_ya_seleccionado_sin_arrastrar_no_deja_paso_fantasma() {
        // La instantanea de apuntar_edicion() se toma al pulsar (para que
        // el arrastre que sigue no asigne memoria), pero un clic sin
        // arrastre no cambia nada. Si ese paso entrara igual en el
        // historial, Ctrl+Z no deshaceria nada visible y habria que
        // pulsarlo dos veces para llegar al cambio de verdad.
        let mut escena = Escena::nueva();
        let id = escena.anadir(rect(0.0, 0.0, 100.0, 100.0));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        g.evento(pulsar(Punto2::nuevo(50.0, 50.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(50.0, 50.0),
            },
            &mut escena,
            1.0,
        );

        assert!(escena.deshacer(), "queda el paso de haberlo anadido");
        assert!(
            escena.buscar(id).unwrap().borrado,
            "y es el unico paso: el clic no dejo nada de por medio"
        );
        assert!(!escena.deshacer(), "no hay paso fantasma del clic");
    }

    #[test]
    fn el_cursor_de_escalar_va_girado_con_el_elemento() {
        // En una figura a 45 grados, el tirador de la esquina ensena la
        // flecha que de verdad apunta hacia donde va a crecer.
        use std::f32::consts::FRAC_PI_4;
        let mut escena = Escena::nueva();
        let mut e = rect(0.0, 0.0, 100.0, 100.0);
        e.angulo = FRAC_PI_4;
        let id = escena.anadir(e);
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Mano;
        g.seleccion.poner(id);

        let ts = Tiradores::de_elemento(escena.buscar(id).unwrap(), 1.0);
        let se = ts
            .tamano
            .iter()
            .find(|(c, _)| *c == Tirador::SuresteEsquina)
            .unwrap()
            .1;
        let r = g.evento(mover(se), &mut escena, 1.0);

        let FormaCursor::Escalar { tirador, angulo } = r.cursor else {
            panic!("sobre un tirador toca cursor de escalar, es {:?}", r.cursor);
        };
        assert_eq!(
            tirador,
            Tirador::SuresteEsquina,
            "cual, para saber la direccion"
        );
        assert!(
            (angulo - FRAC_PI_4).abs() < 1e-3,
            "y el angulo del elemento, para girarla: {angulo}"
        );
    }

    #[test]
    fn la_direccion_de_un_tirador_gira_con_el_elemento() {
        // Sin girar, la esquina sureste apunta a 135 grados (abajo y a la
        // derecha). Girado un cuarto de vuelta, apunta a 225.
        use std::f32::consts::{FRAC_PI_2, PI};
        let recta = direccion_del_tirador(Tirador::SuresteEsquina, 0.0);
        assert!((recta - 3.0 * PI / 4.0).abs() < 1e-3, "sin girar: {recta}");

        let girada = direccion_del_tirador(Tirador::SuresteEsquina, FRAC_PI_2);
        assert!((girada - 5.0 * PI / 4.0).abs() < 1e-3, "girada: {girada}");
    }

    #[test]
    fn escalar_pide_la_medida_al_soltar() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 0.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        let Some(Peticion::Calibrar { largo_px }) = r.pide else {
            panic!("al soltar tiene que pedir la medida, dio {:?}", r.pide);
        };
        assert!((largo_px - 100.0).abs() < 1e-3);
    }

    #[test]
    fn shift_sujeta_la_raya_de_calibrar_a_horizontal_o_vertical() {
        // Diseno S5: «con Shift se sujeta a horizontal o vertical. Calibrar
        // sobre el borde de una pared es el caso normal, y a pulso sale
        // torcida: un grado de mas son dos centimetros de error por
        // metro». `Estado::Calibrando` recibia `shift` y no lo miraba.
        //
        // `restringir_angulo` conserva el radio y solo redondea el angulo
        // -el mismo mecanismo que `Linea`-, asi que `largo_px` (la distancia
        // al origen) no varia al sujetar: lo que cambia es HACIA DONDE cae
        // el punto. Por eso la prueba mira el ultimo punto del `trazo`
        // -campo privado, pero este modulo de pruebas es descendiente de
        // `gesto` y lo alcanza- y no `Peticion::Calibrar`.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        // Casi horizontal, pero no del todo: sin sujetar, el ultimo punto
        // del trazo seria justo este, con y = 10.0.
        g.evento(
            EventoGesto::Mover {
                p: Punto2::nuevo(100.0, 10.0),
                shift: true,
                alt: false,
                presion: None,
            },
            &mut escena,
            1.0,
        );

        let ultimo = *g.trazo.last().expect("el trazo tiene al menos un punto");
        assert!(
            ultimo.y.abs() < 1e-2,
            "sujeto a horizontal, y tiene que caer en 0: salio {ultimo:?}"
        );
    }

    #[test]
    fn sin_shift_la_raya_de_calibrar_no_se_sujeta() {
        // Caso negativo del anterior: sin Shift el punto se queda tal cual
        // llega, torcido incluido -es justo lo que Shift existe para poder
        // evitar cuando se quiere.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 10.0)), &mut escena, 1.0);

        let ultimo = *g.trazo.last().expect("el trazo tiene al menos un punto");
        assert!(
            (ultimo.y - 10.0).abs() < 1e-3,
            "sin Shift no se sujeta: y tenia que seguir en 10, salio {ultimo:?}"
        );
    }

    #[test]
    fn el_umbral_de_calibrar_son_dos_pixeles_de_pantalla_y_no_de_mundo() {
        // Hallazgo 9: `largo_px` viaja en unidades de MUNDO, y el umbral se
        // comparaba contra 2.0 a pulso -dos unidades de mundo, no dos
        // pixeles de pantalla. Con un zoom que aleja la camara, `escala`
        // (unidades de mundo por pixel) crece, y dos pixeles de pantalla
        // son mas de dos unidades de mundo.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // 3 unidades de mundo: mas de dos unidades de mundo a pulso, pero
        // con un zoom al 10% (escala = 10.0 unidades de mundo por pixel)
        // son solo 0,3 pixeles de pantalla, menos que el umbral de dos.
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 10.0);
        g.evento(mover(Punto2::nuevo(3.0, 0.0)), &mut escena, 10.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(3.0, 0.0),
            },
            &mut escena,
            10.0,
        );
        assert_eq!(
            r.pide, None,
            "3 unidades de mundo con escala 10 son 0,3 px de pantalla: no llega al umbral"
        );

        // La misma distancia de mundo, con la camara a tamano natural
        // (escala = 1.0): ahora si son tres pixeles de pantalla, por
        // encima del umbral.
        let mut g2 = Gesto::nuevo();
        g2.herramienta = Herramienta::Escalar;
        g2.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g2.evento(mover(Punto2::nuevo(3.0, 0.0)), &mut escena, 1.0);
        let r2 = g2.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(3.0, 0.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            r2.pide.is_some(),
            "3 unidades de mundo con escala 1 son 3 px de pantalla: llega al umbral"
        );
    }

    #[test]
    fn la_raya_de_calibrar_no_se_queda_en_el_dibujo() {
        // Era un metro, no un dibujo. Si se quedara, cada calibrado dejaria
        // basura en el lienzo.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, 0.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 0.0),
            },
            &mut escena,
            1.0,
        );

        assert_eq!(escena.cuantos_visibles(), 0, "la raya se fue");
    }

    #[test]
    fn calibrar_pone_la_escala_en_la_escena() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(g.calibrar(&mut escena, 100.0, 3.0, "m"));
        let e = escena.escala.as_ref().expect("hay escala");
        assert!((e.unidades_por_pixel - 0.03).abs() < 1e-6);
        assert_eq!(e.unidad, "m");
    }

    #[test]
    fn calibrar_con_una_medida_imposible_no_toca_la_escena() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(
            !g.calibrar(&mut escena, 100.0, 0.0, "m"),
            "devuelve que no pudo"
        );
        assert!(escena.escala.is_none(), "y no deja nada a medias");
    }

    #[test]
    fn recalibrar_sustituye_la_escala_anterior() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.calibrar(&mut escena, 100.0, 3.0, "m");
        g.calibrar(&mut escena, 100.0, 6.0, "m");
        assert!((escena.escala.unwrap().unidades_por_pixel - 0.06).abs() < 1e-6);
    }

    #[test]
    fn calibrar_se_puede_deshacer() {
        // Calibrar mal y no poder volver atras seria tener que rehacer el
        // lienzo entero.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.calibrar(&mut escena, 100.0, 3.0, "m");
        assert!(escena.escala.is_some());
        assert!(escena.deshacer());
        assert!(escena.escala.is_none(), "vuelve a estar sin calibrar");
    }

    #[test]
    fn la_cota_deja_una_cota_y_un_paso_de_deshacer() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(matches!(
            escena.visibles().next().unwrap().figura,
            Figura::Cota { .. }
        ));
        assert!(escena.deshacer());
        assert_eq!(escena.cuantos_visibles(), 0);
    }

    #[test]
    fn la_cota_deja_los_puntos_donde_se_arrastro() {
        // Una cota con los puntos mal mide mal: "una cota no puede mentir"
        // es la propiedad que justifica el diseno de toda esta fase. No
        // basta con comprobar que hay un elemento y que es una Cota (eso ya
        // lo hace `la_cota_deja_una_cota_y_un_paso_de_deshacer`); hace falta
        // comprobar que el primer punto es donde se pulso y el segundo el
        // que sigue al cursor, tal como hace `mover` para Linea y Flecha.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;

        // Un arrastre real trae varios `Mover` antes de soltar, no uno
        // solo: si el segundo punto se acumulase en vez de sustituirse (el
        // fallo que esta prueba busca), harian falta varios avisos de
        // movimiento para notarlo.
        g.evento(pulsar(Punto2::nuevo(10.0, 20.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(60.0, 20.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(80.0, 20.0)), &mut escena, 1.0);
        // `soltar` no toca la geometria: el ultimo punto es el del ultimo
        // `Mover`, asi que el arrastre real termina con uno en el mismo
        // sitio donde se suelta.
        g.evento(mover(Punto2::nuevo(110.0, 20.0)), &mut escena, 1.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(110.0, 20.0),
            },
            &mut escena,
            1.0,
        );

        let Figura::Cota { puntos } = &escena.visibles().next().unwrap().figura else {
            panic!("tiene que ser una Cota");
        };
        assert_eq!(
            puntos.len(),
            2,
            "solo dos puntos, no uno por cada aviso de movimiento"
        );
        assert_eq!(
            puntos[0],
            Punto2::nuevo(10.0, 20.0),
            "el primer punto es donde se pulso"
        );
        assert_eq!(
            puntos[1],
            Punto2::nuevo(110.0, 20.0),
            "el segundo sigue al cursor, no se acumula"
        );
    }

    #[test]
    fn la_cota_funciona_sin_haber_calibrado() {
        // D35: sin escala mide en pixeles. La herramienta no se bloquea.
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Cota;
        assert!(escena.escala.is_none());

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(100.0, 0.0),
        );
        assert_eq!(escena.cuantos_visibles(), 1, "deja la cota igual");
    }

    #[test]
    fn la_escala_grafica_deja_una_barra() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::EscalaGrafica;

        arrastrar(
            &mut g,
            &mut escena,
            Punto2::nuevo(0.0, 0.0),
            Punto2::nuevo(400.0, 24.0),
        );

        assert_eq!(escena.cuantos_visibles(), 1);
        assert!(matches!(
            escena.visibles().next().unwrap().figura,
            Figura::EscalaGrafica
        ));
    }

    #[test]
    fn las_herramientas_de_medir_dejan_rastro_menos_escalar() {
        assert!(Herramienta::Cota.deja_rastro());
        assert!(Herramienta::EscalaGrafica.deja_rastro());
        assert!(
            !Herramienta::Escalar.deja_rastro(),
            "la raya de calibrar no se guarda"
        );
    }

    #[test]
    fn los_gestos_que_no_piden_nada_no_piden_nada() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;
        let r = g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        assert!(r.pide.is_none());
    }

    /// Un rectangulo de 100x50 en (0,0) con el id que se pida.
    ///
    /// `Elemento` no tiene constructora: se monta con el literal entero,
    /// igual que hacen las pruebas vecinas de este fichero.
    fn rect_para_iman(id: u64) -> Elemento {
        Elemento {
            figura: Figura::Rectangulo,
            id,
            x: 0.0,
            y: 0.0,
            ancho: 100.0,
            alto: 50.0,
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

    #[test]
    fn trazar_una_figura_engancha_a_la_esquina_de_otra() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        // Se pulsa a 10 de escena de la esquina (100,50): fuera del margen
        // de picado de `impacto::toca` (grosor/2 + 6 = 7, asi que el clic
        // no selecciona el vecino en vez de dibujar) y dentro del radio del
        // iman (14).
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        let a = g.anclaje_activo.expect("tenia que enganchar");
        assert_eq!(a.punto, Punto2::nuevo(100.0, 50.0));
        assert_eq!(a.id, 7);
    }

    #[test]
    fn el_lapiz_no_engancha_aunque_pase_por_encima_de_un_vertice() {
        // La guarda de `Faena::AMano`. Un trazo que salta a un vertice en
        // mitad del recorrido no se corrige, se rompe.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Lapiz;

        // A 10 de la esquina (100,50) y no a 4,24: dentro del margen de
        // picado de `impacto::toca` (grosor/2 + 6 = 7) el clic selecciona el
        // rectangulo y ni llega a la rama del lapiz, asi que la prueba
        // pasaria sin ejercitar `Faena::AMano` -que es todo lo que aqui
        // importa-.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);
        assert!(
            escena.elementos.len() == 2 && g.seleccion.ids().is_empty(),
            "el clic tiene que hacer nacer un trazo, no seleccionar al vecino"
        );
        // Aislada aqui: cubre la rama `Estado::Reposo` de `faena()`, que el
        // `Mover` de abajo ya no toca -ese pasa por `Estado::Dibujando`-.
        assert!(
            g.anclaje_activo.is_none(),
            "en reposo el lapiz tampoco engancha"
        );

        // Y ahora dentro del trazo, con el cursor a 1,41 del vertice ajeno:
        // esta es la rama `Estado::Dibujando`. El elemento que nace se
        // excluye a si mismo, pero el rectangulo 7 no esta excluido, asi que
        // lo unico que puede devolver `None` aqui es `Faena::AMano`.
        g.evento(mover(Punto2::nuevo(101.0, 51.0)), &mut escena, 1.0);
        assert!(
            g.anclaje_activo.is_none(),
            "el lapiz no puede pegar tirones a mitad de trazo"
        );
    }

    #[test]
    fn la_figura_que_nace_no_se_engancha_a_si_misma() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        // Al arrastrar, la esquina del propio rectangulo que esta naciendo
        // queda justo bajo el cursor.
        g.evento(mover(Punto2::nuevo(40.0, 40.0)), &mut escena, 1.0);

        let nacido = escena.elementos.last().expect("nacio algo").id;
        assert!(
            g.anclaje_activo.is_none_or(|a| a.id != nacido),
            "se ha pegado a su propia esquina"
        );
    }

    #[test]
    fn calibrar_engancha_a_los_extremos() {
        // La fila que justifica la fase: picar los dos extremos de una pared
        // de medida conocida deja de ser punteria.
        //
        // El arrastre entero -pulsar, mover, soltar- y no un solo `pulsar`:
        // el segundo punto, el que de verdad fija la medida, viaja por
        // `mover` en `Estado::Calibrando`, que es la unica rama de `faena()`
        // que el diseno llama «la fila que justifica la fase». Y se asere
        // sobre `Respuesta.pide`, que es lo observable: `anclaje_activo` se
        // escribe antes de que la maquina de estados haga nada, asi que no
        // dice si el punto enganchado sirvio para algo.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // El rectangulo va de (0,0) a (100,50). Se pica a 11 a la izquierda
        // de la esquina (0,0) y se suelta a 12 por encima de la (100,0): las
        // dos fuera del margen de picado de `impacto::toca` (grosor/2 + 6 =
        // 7, asi que el clic no selecciona el vecino) y dentro del radio del
        // iman (14). Y a proposito descuadradas: a pulso el largo saldria
        // 111,6, asi que 100 solo puede venir de los dos enganches.
        g.evento(pulsar(Punto2::nuevo(-11.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(100.0, -12.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, -12.0),
            },
            &mut escena,
            1.0,
        );

        // Con los dos extremos enganchados el largo es exactamente 100: la
        // distancia de (0,0) a (100,0).
        match r.pide {
            Some(Peticion::Calibrar { largo_px }) => assert!(
                (largo_px - 100.0).abs() < 0.01,
                "el iman no llevo los dos extremos al vertice: {largo_px}"
            ),
            otro => panic!("calibrar tenia que pedir la medida, y pidio {otro:?}"),
        }
        assert!(
            g.seleccion.ids().is_empty(),
            "calibrar no puede acabar seleccionando el vecino del que se engancha"
        );
    }

    #[test]
    fn con_el_iman_encendido_calibrar_sigue_calibrando_y_no_selecciona() {
        // El circulo vicioso que esto cierra: todo anclaje de esquina,
        // extremo o medio cae JUSTO sobre la geometria del elemento, asi que
        // si el punto se engancha antes de las cribas de `pulsar`, el paso 3
        // (`elemento_en`) da positivo con certeza y el clic se convierte en
        // «seleccionar y mover el vecino». Encender el iman -que es el
        // estado de fabrica- rompia justo la fila que justifica la fase.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Escalar;

        // (0,-12): a 12 de la esquina (0,0), fuera del margen de picado (7)
        // y dentro del radio del iman (14).
        g.evento(pulsar(Punto2::nuevo(0.0, -12.0)), &mut escena, 1.0);
        assert!(
            g.seleccion.ids().is_empty(),
            "el iman robo el clic y selecciono el rectangulo de al lado"
        );
        g.evento(mover(Punto2::nuevo(100.0, 60.0)), &mut escena, 1.0);
        let r = g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(100.0, 60.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            matches!(r.pide, Some(Peticion::Calibrar { .. })),
            "con el iman encendido calibrar deja de calibrar: {:?}",
            r.pide
        );
    }

    #[test]
    fn con_el_iman_encendido_una_figura_nace_en_la_esquina_de_otra() {
        // La otra mitad del mismo fallo: empezar un rectangulo pegado a la
        // esquina del vecino seleccionaba al vecino en vez de hacer nacer
        // nada.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        // A 10 de la esquina (100,50) por fuera: fuera del margen de picado
        // (7), dentro del radio del iman (14).
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        assert_eq!(
            escena.elementos.len(),
            2,
            "no nacio ningun elemento: el clic se lo comio la seleccion"
        );
        assert!(
            g.seleccion.ids().is_empty(),
            "dibujar no puede acabar seleccionando el vecino"
        );
        let nacido = escena.elementos.last().expect("nacio algo");
        assert_eq!(
            (nacido.x, nacido.y),
            (100.0, 50.0),
            "y nace pegado a la esquina del vecino, que es para lo que esta el iman"
        );
    }

    fn dibujar_a_mano(g: &mut Gesto, escena: &mut Escena, puntos: &[Punto2]) {
        g.herramienta = Herramienta::Lapiz;
        g.enganche.activo = false;
        g.evento(pulsar(puntos[0]), escena, 1.0);
        for p in &puntos[1..] {
            g.evento(mover(*p), escena, 1.0);
        }
    }

    #[test]
    fn un_circulo_a_mano_quieto_se_vuelve_elipse_y_crece_desde_su_centro() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let circulo: Vec<Punto2> = (0..=48)
            .map(|i| {
                let t = std::f32::consts::TAU * i as f32 / 48.0;
                Punto2::nuevo(100.0 + 50.0 * t.cos(), 100.0 + 50.0 * t.sin())
            })
            .collect();
        dibujar_a_mano(&mut g, &mut escena, &circulo);
        let cursor = *circulo.last().unwrap();
        assert!(g.convertir_en_forma(&mut escena, cursor).is_some());
        let (id, _) = g.elemento_en_curso().unwrap();
        assert!(matches!(escena.buscar(id).unwrap().figura, Figura::Elipse));
        // Alejarse al doble del centro dobla los radios sin mover el centro.
        g.evento(mover(Punto2::nuevo(200.0, 100.0)), &mut escena, 1.0);
        let e = escena.buscar(id).unwrap();
        assert!((e.ancho - 200.0).abs() < 4.0, "ancho {}", e.ancho);
        assert!(
            ((e.x + e.ancho / 2.0) - 100.0).abs() < 2.0,
            "el centro no se mueve"
        );
    }

    #[test]
    fn una_l_quieta_se_vuelve_rectangulo_que_se_ajusta_al_tirar_y_se_deshace_de_una() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        let mut l: Vec<Punto2> = (0..=20)
            .map(|i| Punto2::nuevo(0.0, 100.0 - 5.0 * i as f32))
            .collect();
        l.extend((1..=30).map(|i| Punto2::nuevo(5.0 * i as f32, 0.0)));
        dibujar_a_mano(&mut g, &mut escena, &l);
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(150.0, 0.0))
                .is_some()
        );
        let (id, _) = g.elemento_en_curso().unwrap();
        // Tirar hacia abajo a la derecha: la esquina opuesta (abajo a la
        // izquierda, donde empezo) se queda fija.
        g.evento(mover(Punto2::nuevo(220.0, -40.0)), &mut escena, 1.0);
        let e = escena.buscar(id).unwrap().clone();
        assert!(matches!(e.figura, Figura::Rectangulo));
        assert_eq!((e.x, e.y + e.alto), (0.0, 100.0), "el ancla es el inicio");
        assert_eq!(e.ancho, 220.0);
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(220.0, -40.0),
            },
            &mut escena,
            1.0,
        );
        assert!(escena.deshacer());
        assert!(
            escena.buscar(id).is_none_or(|e| e.borrado),
            "un Ctrl+Z lo quita entero"
        );
    }

    #[test]
    fn un_garabato_quieto_no_se_convierte_y_sin_dibujar_tampoco() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(0.0, 0.0))
                .is_none()
        );
        let zig: Vec<Punto2> = (0..40)
            .map(|i| Punto2::nuevo(i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 30.0 }))
            .collect();
        dibujar_a_mano(&mut g, &mut escena, &zig);
        assert!(
            g.convertir_en_forma(&mut escena, Punto2::nuevo(195.0, 30.0))
                .is_none()
        );
    }

    #[test]
    fn arrastrar_un_rectangulo_repinta_la_figura_entera_y_no_solo_el_cursor() {
        let mut escena = Escena::nueva();
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        g.enganche.activo = false;
        g.evento(pulsar(Punto2::nuevo(0.0, 0.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(300.0, 200.0)), &mut escena, 1.0);
        let r = g.evento(mover(Punto2::nuevo(310.0, 210.0)), &mut escena, 1.0);
        match r.region {
            Region::Caja(x0, y0, x1, y1) => {
                // El lado izquierdo y el de arriba nacen en el origen y
                // cambian con cada movimiento: tienen que entrar en la zona.
                assert!(x0 <= 0.0 && y0 <= 0.0, "deja fuera el origen: {x0},{y0}");
                assert!(
                    x1 >= 310.0 && y1 >= 210.0,
                    "deja fuera el cursor: {x1},{y1}"
                );
            }
            otra => panic!("esperaba una caja, llego {otra:?}"),
        }
    }

    #[test]
    fn soltar_apaga_la_pista() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;

        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);
        assert!(g.anclaje_activo.is_some());
        g.evento(
            EventoGesto::Soltar {
                p: Punto2::nuevo(150.0, 90.0),
            },
            &mut escena,
            1.0,
        );
        assert!(
            g.anclaje_activo.is_none(),
            "la marca se queda pintada despues de soltar"
        );
    }

    #[test]
    fn con_el_iman_apagado_el_punto_llega_intacto() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        g.enganche = crate::enganche::Ajustes::NINGUNO;

        // A distancia intermedia de la esquina (100,50): a 10, fuera del
        // margen de picado normal de `impacto::toca` (grosor/2 + 6 = 7, asi
        // que el clic no selecciona el rectangulo 7 en vez de dibujar uno
        // nuevo) pero dentro del radio del iman (14 a esta escala). Un punto
        // mas lejos -(300,300), por ejemplo- no distinguiria "iman apagado"
        // de "no habia nada que enganchar": ahi tampoco engancharia con el
        // iman encendido, y la prueba pasaria igual con `self.enganche` roto.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 1.0);

        assert!(g.anclaje_activo.is_none());
        let nuevo = escena.elementos.last().expect("nacio un rectangulo");
        assert_eq!(nuevo.x, 110.0, "el iman apagado no puede mover el punto");
    }

    #[test]
    fn la_marquesina_no_engancha() {
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        // `Mano` es la herramienta de seleccionar y mover: no hay variante
        // `Seleccion` en el enum.
        g.herramienta = Herramienta::Mano;

        // Se pulsa en vacio, lejos del rectangulo, y se arrastra hacia su
        // esquina: seleccionar no es trazar, no debe pegarse.
        g.evento(pulsar(Punto2::nuevo(300.0, 300.0)), &mut escena, 1.0);
        g.evento(mover(Punto2::nuevo(102.0, 52.0)), &mut escena, 1.0);

        assert!(g.anclaje_activo.is_none());
    }

    #[test]
    fn enganchar_convierte_la_escala_a_zoom_no_la_pasa_directa() {
        // `evento` recibe `escala` -unidades de escena por pixel de
        // pantalla, `1.0 / camara.zoom`-, y el iman quiere el zoom. Las
        // pruebas de arriba usan todas escala 1.0, y el inverso de 1 es 1:
        // si `enganchar` pasara `escala` directa a `sitio()` como si fuera
        // el zoom, esta prueba es la unica que lo notaria.
        //
        // A escala 0.25 el zoom es 4 y el radio de escena queda en
        // 14/4=3.5: un punto a 10 de la esquina no debe enganchar. Si se
        // pasara la escala sin invertir, "zoom" seria 0.25 y el radio
        // saldria en 14/0.25=56, y si engancharia.
        let mut escena = Escena::nueva();
        escena.elementos.push(rect_para_iman(7));
        let mut g = Gesto::nuevo();
        g.herramienta = Herramienta::Rectangulo;
        // (110,50) y no (10,0): a 10 de la esquina (100,50) por fuera del
        // rectangulo, o sea fuera del margen de picado (7). Sobre la propia
        // geometria el clic seleccionaria el vecino y ni llegaria al iman.
        g.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena, 0.25);
        assert!(
            g.anclaje_activo.is_none(),
            "a escala 0.25 (zoom 4, radio de escena 3.5) 10 no puede enganchar"
        );

        // El mismo punto, a escala 1.0 (zoom 1, radio de escena 14), si
        // debe enganchar: confirma que la conversion no rompe el caso
        // normal, solo el invertido.
        let mut escena2 = Escena::nueva();
        escena2.elementos.push(rect_para_iman(7));
        let mut g2 = Gesto::nuevo();
        g2.herramienta = Herramienta::Rectangulo;
        g2.evento(pulsar(Punto2::nuevo(110.0, 50.0)), &mut escena2, 1.0);
        assert!(
            g2.anclaje_activo.is_some(),
            "a escala 1.0 (zoom 1, radio de escena 14) 10 tiene que enganchar"
        );
    }
}
