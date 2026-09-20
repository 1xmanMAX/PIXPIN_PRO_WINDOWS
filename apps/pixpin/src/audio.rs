//! **El reproductor de la aplicacion: uno para todo.**
//!
//! Es el `object Reproductor` del movil (`guardados/Reproductor.kt:9-19`) y
//! el razonamiento se copia entero: antes cada chat tenia su reproductor y
//! una nota de voz era un boton de play y poco mas —ni adelantar, ni
//! atrasar, ni velocidad— y al salir del chat se cortaba. Aqui vive el
//! estado —que suena, por donde va, a que velocidad— en un solo sitio, y lo
//! leen la barra de arriba, las burbujas del chat y la biblioteca.
//!
//! ## Por que un `thread_local` y no un `static`
//!
//! Lo que hay debajo es un `IMFMediaEngine`, que es un objeto COM del hilo
//! que lo creo. Un `static` global obligaria a un cerrojo y a prometer que
//! es seguro pasarlo entre hilos, que no lo es. La aplicacion reproduce
//! desde el hilo de la interfaz y solo desde ahi —es donde estan los botones
//! y la lista—, asi que el singleton vive en ese hilo y el compilador lo
//! vigila gratis.
//!
//! ## Lo que este modulo NO hace
//!
//! No pinta nada y no sabe de rotulos: la barra
//! (`BarraDelReproductor.kt:46-106`) y la pantalla de la letra viven en
//! `pixpin-ui`/`ventana_chat.rs` y se enganchan en la tanda de costura.
//! Aqui solo esta el estado y las ordenes.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use pixpin_audio::reloj::{self, Estado};
use pixpin_audio::salida::Salida;
use pixpin_audio::{ErrorAudio, SALTO_MS};

thread_local! {
    /// El unico reproductor. Nace vacio y no abre la tarjeta hasta que
    /// alguien toca una nota.
    static REPRODUCTOR: RefCell<Reproductor> = RefCell::new(Reproductor::default());
}

#[derive(Default)]
struct Reproductor {
    salida: Option<Salida>,
    estado: Estado,
}

/// Lo que se sabe de lo que suena ahora mismo.
///
/// Devuelve una copia: quien pinta la lee y la suelta, y asi no se queda
/// nadie con el `RefCell` cogido mientras dibuja.
pub fn estado() -> Estado {
    REPRODUCTOR.with(|r| r.borrow().estado.clone())
}

/// Si hay algo cargado. La barra no se pinta cuando no lo hay
/// (`BarraDelReproductor.kt:48`).
pub fn hay_algo() -> bool {
    REPRODUCTOR.with(|r| r.borrow().estado.ruta.is_some())
}

/// Toca lo que ya suena: pausa o sigue. Toca otra cosa: la carga y arranca.
///
/// `Reproductor.alternar` (`Reproductor.kt:54-61`). Es lo que hace un clic
/// en la burbuja de una nota de voz y un clic en una fila de la biblioteca.
pub fn alternar(ruta: &Path, titulo: &str) -> Result<(), ErrorAudio> {
    let misma = REPRODUCTOR.with(|r| {
        let r = r.borrow();
        r.salida.is_some() && r.estado.ruta.as_deref() == Some(ruta)
    });
    if misma {
        if estado().sonando {
            pausar();
        } else {
            seguir()?;
        }
        return Ok(());
    }
    cargar(ruta, titulo, true)
}

/// Carga un audio, y opcionalmente arranca.
///
/// Suelta lo que hubiera antes: **uno solo a la vez**, como en el movil
/// (`Reproductor.cargar` :63-79). La velocidad **sobrevive** al cambio de
/// pista: quien escucha todo a `1,5x` no quiere volver a ponerlo.
pub fn cargar(ruta: &Path, titulo: &str, arrancar: bool) -> Result<(), ErrorAudio> {
    let velocidad = estado().velocidad;
    parar();

    let salida = Salida::abrir(ruta)?;
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        r.estado = Estado {
            ruta: Some(ruta.to_path_buf()),
            titulo: titulo.to_string(),
            sonando: false,
            posicion_ms: 0,
            // Todavia no se sabe: la carga es asincrona y la duracion
            // aparece en el primer latido que la encuentre.
            duracion_ms: 0,
            velocidad,
        };
        r.salida = Some(salida);
    });

    if arrancar { seguir() } else { Ok(()) }
}

/// Arranca o sigue.
pub fn seguir() -> Result<(), ErrorAudio> {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        let velocidad = r.estado.velocidad;
        let Some(s) = r.salida.as_ref() else {
            return Ok(());
        };
        s.tocar(velocidad)?;
        r.estado.sonando = true;
        Ok(())
    })
}

pub fn pausar() {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        if let Some(s) = r.salida.as_ref() {
            s.pausar();
            let donde = s.posicion_ms();
            r.estado.posicion_ms = donde;
        }
        r.estado.sonando = false;
    });
}

/// Descarga lo que hubiera: la barra desaparece y se suelta la tarjeta.
///
/// Conserva la velocidad (`Reproductor.parar` :101-106).
pub fn parar() {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        // El `Drop` de `Salida` llama a `Shutdown`: soltarla es soltar el
        // motor y sus hilos.
        r.salida = None;
        r.estado = Estado::vacio_con_velocidad(r.estado.velocidad);
    });
}

/// Adelanta o atrasa. `SALTO_MS` en negativo para atras.
///
/// `Reproductor.saltar` (:109-115).
pub fn saltar(ms: i64) {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        let destino = reloj::destino_al_saltar(r.estado.posicion_ms, ms, r.estado.duracion_ms);
        if let Some(s) = r.salida.as_ref() {
            s.ir_a_ms(destino);
        }
        r.estado.posicion_ms = destino;
    });
}

/// Los diez segundos de los botones de la barra.
pub fn atras() {
    saltar(-SALTO_MS);
}

pub fn adelante() {
    saltar(SALTO_MS);
}

/// Va a un punto de la barra, de 0 a 1.
///
/// `Reproductor.irA` (:118-125). Sin duracion todavia no hace nada: pinchar
/// en la mitad de una barra que aun no sabe cuanto mide no significa nada, y
/// mandar a cero seria rebobinar sin que nadie lo pidiera.
pub fn ir_a(fraccion: f32) {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        let Some(destino) = reloj::destino_de_fraccion(fraccion, r.estado.duracion_ms) else {
            return;
        };
        if let Some(s) = r.salida.as_ref() {
            s.ir_a_ms(destino);
        }
        r.estado.posicion_ms = destino;
    });
}

/// La siguiente de las **cinco** velocidades, en ciclo.
///
/// `Reproductor.otraVelocidad` (:128-135). Devuelve la nueva para poder
/// rotular el boton sin volver a preguntar por el estado entero.
pub fn otra_velocidad() -> f32 {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        let nueva = reloj::siguiente_velocidad(r.estado.velocidad);
        r.estado.velocidad = nueva;
        if let Some(s) = r.salida.as_ref() {
            s.poner_velocidad(nueva);
        }
        nueva
    })
}

/// El latido: se llama cada [`reloj::MS_ENTRE_LATIDOS`] desde el temporizador
/// de la ventana.
///
/// Hace tres cosas y las tres tienen que pasar aqui y no en un hilo aparte:
/// recoge la duracion en cuanto Media Foundation la sabe (la carga es
/// asincrona), copia por donde va, y rebobina al llegar al final, que es el
/// `setOnCompletionListener` del movil (`Reproductor.kt:69-72`).
///
/// Devuelve `true` si hay que repintar la barra.
pub fn latido() -> bool {
    REPRODUCTOR.with(|r| {
        let mut r = r.borrow_mut();
        let Some(s) = r.salida.as_ref() else {
            return false;
        };

        if s.fallo() {
            // Un fichero que Media Foundation no sabe abrir se descarga: la
            // barra desaparece en vez de quedarse parada en 0:00 para
            // siempre.
            r.salida = None;
            r.estado = Estado::vacio_con_velocidad(r.estado.velocidad);
            return true;
        }

        // Se pregunta todo a la tarjeta **antes** de tocar el estado: el
        // prestamo de `salida` y el de `estado` salen del mismo `RefCell`,
        // y mezclarlos no compila.
        let duracion = s.duracion_ms();
        let termino = s.termino();
        let sonando = r.estado.sonando;
        let donde = if sonando { s.posicion_ms() } else { 0 };
        if termino {
            s.pausar();
            s.ir_a_ms(0);
        }

        if termino {
            r.estado.duracion_ms = duracion;
            r.estado.sonando = false;
            r.estado.posicion_ms = 0;
            return true;
        }

        let mut cambio = false;
        if duracion != r.estado.duracion_ms {
            r.estado.duracion_ms = duracion;
            cambio = true;
        }
        if sonando && donde != r.estado.posicion_ms {
            r.estado.posicion_ms = donde;
            cambio = true;
        }
        cambio
    })
}

/// Si el audio que suena es este. Lo pregunta cada burbuja del chat para
/// saber si pinta el boton de pausa o el de play.
pub fn suena(ruta: &Path) -> bool {
    REPRODUCTOR.with(|r| {
        let r = r.borrow();
        r.estado.sonando && r.estado.ruta.as_deref() == Some(ruta)
    })
}

/// La ruta de lo que esta cargado, suene o no.
pub fn cargado() -> Option<PathBuf> {
    REPRODUCTOR.with(|r| r.borrow().estado.ruta.clone())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Las pruebas de este modulo no abren la tarjeta: comprueban el estado
    /// cuando **no hay nada cargado**, que es donde estan los fallos de
    /// verdad (ordenes que llegan antes de que haya audio y revientan).
    #[test]
    fn sin_nada_cargado_las_ordenes_no_revientan_ni_inventan_estado() {
        parar();
        assert!(!hay_algo());
        assert!(!latido());
        pausar();
        saltar(SALTO_MS);
        ir_a(0.5);
        seguir().unwrap();
        let e = estado();
        assert!(e.ruta.is_none());
        assert!(!e.sonando);
        assert_eq!(e.posicion_ms, 0);
        assert_eq!(e.duracion_ms, 0);
    }

    #[test]
    fn la_velocidad_cicla_y_sobrevive_a_parar() {
        parar();
        assert_eq!(estado().velocidad, 1.0);
        assert_eq!(otra_velocidad(), 1.25);
        assert_eq!(otra_velocidad(), 1.5);
        parar();
        assert_eq!(
            estado().velocidad,
            1.5,
            "parar no puede olvidar la velocidad elegida"
        );
        // Y sigue el ciclo donde estaba.
        assert_eq!(otra_velocidad(), 2.0);
        assert_eq!(otra_velocidad(), 0.75);
        assert_eq!(otra_velocidad(), 1.0);
    }

    #[test]
    fn un_fichero_que_no_existe_no_deja_la_barra_puesta() {
        parar();
        let e = cargar(Path::new("C:/no/existe/nada.m4a"), "nada", true).unwrap_err();
        assert!(matches!(e, ErrorAudio::NoEsAudio { .. }), "salio {e:?}");
        assert!(!hay_algo(), "quedo una barra apuntando a un hueco");
    }
}
