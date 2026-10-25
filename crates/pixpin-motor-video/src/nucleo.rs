//! **El nucleo del motor**: lo que decide, en cualquier sistema, que
//! fotograma se codifica, en que instante, que se hace en pausa, cuando el
//! codificador esta atascado y como se cierra. Habla con la pantalla y el
//! codificador solo por los rasgos de [`crate::plataforma`], asi que se
//! prueba entero en cualquier maquina con una captura y una tuberia de
//! mentira, y es lo que un backend de Android reutilizaria.
//!
//! En cada vuelta del hilo del motor:
//!
//! 1. el sonido que llega se recorta si pisa lo ya escrito
//!    ([`crate::audio::Pista`]), se escribe y ancla el reloj;
//! 2. [`Nucleo::paso`] mira el ritmo ([`crate::ritmo::decidir`]): un
//!    fotograma nuevo (fechado por [`crate::reloj::Reloj`]), el repetido o
//!    nada;
//! 3. lo que no entra cuenta como caido y, si dura [`crate::cola::ATASCO`],
//!    corta el video;
//! 4. al terminar, [`Nucleo::cerrar`] pone el ultimo fotograma hasta donde
//!    acaba el sonido.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use crate::audio::{FormatoAudio, Pista};
use crate::cola::Atasco;
use crate::plataforma::{Captura, Escrito, Retenido, Tuberia};
use crate::reloj::{Reloj, UNIDADES_POR_SEGUNDO};
use crate::ritmo::{self, Decision, Medidor};
use crate::{Codificador, ErrorMotor, Estadisticas, Formato};

/// Con que arranco el motor (no cambia: ni tras una vuelta de la grafica).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arranque {
    pub codificador: Codificador,
    pub formato: Formato,
    pub medidas: (u32, u32),
}

/// **Lo que el hilo cuenta y cualquiera puede leer sin pararlo** (la barra
/// de «Grabar clase», o el lado Kotlin en Android).
#[derive(Debug, Default)]
pub struct Contadores {
    fotogramas: AtomicU64,
    caidos: AtomicU64,
    bytes: AtomicU64,
    fps: AtomicU32,
    cortado: AtomicBool,
    vueltas: AtomicU32,
    arranque: OnceLock<Arranque>,
}

impl Contadores {
    pub fn arrancar(&self, a: Arranque) {
        let _ = self.arranque.set(a);
    }

    pub fn bytes(&self, b: u64) {
        self.bytes.store(b, Ordering::Relaxed);
    }

    pub fn cortar(&self) {
        self.cortado.store(true, Ordering::Release);
    }

    /// Una vuelta de la grafica (se perdio y se volvio a montar).
    pub fn vuelta(&self) {
        self.vueltas.fetch_add(1, Ordering::Relaxed);
    }

    pub fn estadisticas(&self) -> Estadisticas {
        let a = self.arranque.get().copied().unwrap_or(Arranque {
            codificador: Codificador::Cpu,
            formato: Formato::H264,
            medidas: (0, 0),
        });
        Estadisticas {
            fps: f32::from_bits(self.fps.load(Ordering::Relaxed)),
            fotogramas: self.fotogramas.load(Ordering::Relaxed),
            caidos: self.caidos.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            codificador: a.codificador,
            formato: a.formato,
            medidas: a.medidas,
            cortado: self.cortado.load(Ordering::Acquire),
            vueltas: self.vueltas.load(Ordering::Relaxed),
        }
    }
}

/// El estado del video entre vueltas del hilo.
#[derive(Debug)]
pub struct Nucleo {
    audio: Option<FormatoAudio>,
    pista: Pista,
    reloj: Reloj,
    inicio: i64,
    minimo: i64,
    duracion: i64,
    ultimo_intento: Option<i64>,
    medidor: Medidor,
    atasco: Atasco,
    caidos: u64,
}

impl Nucleo {
    /// Un video que empieza a la hora `ahora`, con o sin sonido, a
    /// `por_segundo` como mucho.
    pub fn nuevo(audio: Option<FormatoAudio>, por_segundo: u32, ahora: i64) -> Nucleo {
        Nucleo {
            audio,
            pista: Pista::default(),
            reloj: Reloj::nuevo(audio.map(|a| a.muestreo), ahora),
            inicio: ahora,
            minimo: ritmo::minimo_entre(por_segundo),
            duracion: UNIDADES_POR_SEGUNDO / i64::from(por_segundo.max(1)),
            ultimo_intento: None,
            medidor: Medidor::default(),
            atasco: Atasco::default(),
            caidos: 0,
        }
    }

    /// Lo minimo entre dos fotogramas (para la captura y la tuberia).
    pub fn minimo(&self) -> i64 {
        self.minimo
    }

    pub fn en_pausa(&self) -> bool {
        self.reloj.en_pausa()
    }

    pub fn caidos(&self) -> u64 {
        self.caidos
    }

    /// **Cuanto puede dormir el hilo**, entre 1 y 250 ms: lo que diga el
    /// ritmo, con un techo para que los contadores se refresquen aunque no
    /// pase nada. En pausa, el techo: no hay nada que decidir.
    pub fn espera(&self, ahora: i64, hay_retenido: bool) -> i64 {
        if self.reloj.en_pausa() {
            return UNIDADES_POR_SEGUNDO / 4;
        }
        ritmo::espera(
            ahora,
            self.inicio,
            self.ultimo_intento,
            hay_retenido,
            self.minimo,
        )
        .clamp(10_000, UNIDADES_POR_SEGUNDO / 4)
    }

    /// **Un bloque de sonido** que empieza en `posicion`, llegado a la hora
    /// `ahora`. Lo que pisa lo ya escrito se recorta.
    pub fn audio<T: Tuberia>(
        &mut self,
        tubo: &mut T,
        muestras: &[i16],
        posicion: u64,
        ahora: i64,
    ) -> Result<(), ErrorMotor> {
        let Some(formato) = self.audio else {
            return Ok(());
        };
        let canales = usize::from(formato.canales.max(1));
        let marcos = formato.por_canal(muestras.len());
        let Some((salto, desde)) = self.pista.admitir(posicion, marcos) else {
            tracing::debug!(posicion, marcos, "un bloque de sonido repetido se salta");
            return Ok(());
        };
        if salto > 0 {
            tracing::debug!(salto, "un bloque de sonido pisaba el anterior; se recorta");
        }
        let utiles = &muestras[(salto as usize * canales)..(marcos as usize * canales)];
        tubo.audio(utiles, desde)?;
        self.reloj.sonido(self.pista.fin(), ahora);
        Ok(())
    }

    /// Pausa o sigue a la hora `hora`. Lo retenido en pausa se suelta: al
    /// seguir, la pantalla de antes ya no vale.
    pub fn pausa<C: Captura>(&mut self, captura: Option<&C>, pausar: bool, hora: i64) {
        if pausar {
            self.reloj.pausar(hora);
            if let Some(r) = captura.and_then(Captura::tomar) {
                r.soltar();
            }
        } else {
            self.reloj.seguir(hora);
            self.atasco.reiniciar();
        }
    }

    /// La grafica volvio a montarse: lo que no entro antes no cuenta para
    /// el atasco.
    pub fn tras_una_vuelta(&mut self) {
        self.atasco.reiniciar();
    }

    /// **Un paso del video** a la hora `ahora`: si toca, un fotograma nuevo
    /// o el repetido. Devuelve que paso con el, o `None` si no tocaba.
    pub fn paso<C, T>(
        &mut self,
        ahora: i64,
        captura: &C,
        tubo: &mut T,
        contadores: &Contadores,
    ) -> Result<Option<Escrito>, ErrorMotor>
    where
        C: Captura,
        T: Tuberia<Retenido = C::Retenido>,
    {
        if self.reloj.en_pausa() {
            if let Some(r) = captura.tomar() {
                r.soltar();
            }
            return Ok(None);
        }
        let hora = captura.hora_retenida();
        let escrito = match ritmo::decidir(
            ahora,
            self.inicio,
            self.ultimo_intento,
            hora.is_some(),
            self.minimo,
        ) {
            Decision::Esperar => return Ok(None),
            Decision::Nuevo => {
                self.ultimo_intento = Some(ahora);
                // Si el reloj aun no lo puede fechar (esperando al sonido
                // tras una pausa) se queda retenido: es la imagen mas nueva
                // y no se puede perder.
                let Some(t) = hora.and_then(|h| self.reloj.fotograma(h)) else {
                    return Ok(None);
                };
                let Some(r) = captura.tomar() else {
                    return Ok(None);
                };
                let hecho = tubo.fotograma(&r, t, self.duracion);
                // La tuberia ya tiene en su cola todo lo que lo lee.
                r.soltar();
                hecho?
            }
            Decision::Repetir => {
                self.ultimo_intento = Some(ahora);
                let Some(t) = self.reloj.fotograma(ahora) else {
                    return Ok(None);
                };
                tubo.repetir(t, self.duracion)?
            }
        };
        self.contar(escrito, ahora, tubo.escritos(), contadores)?;
        Ok(Some(escrito))
    }

    fn contar(
        &mut self,
        e: Escrito,
        ahora: i64,
        escritos: u64,
        contadores: &Contadores,
    ) -> Result<(), ErrorMotor> {
        match e {
            Escrito::Hecho => {
                self.atasco.entro();
                self.medidor.uno(ahora);
                contadores.fotogramas.store(escritos, Ordering::Relaxed);
            }
            Escrito::Caido => {
                self.caidos += 1;
                contadores.caidos.store(self.caidos, Ordering::Relaxed);
                if self.atasco.no_entro(ahora) {
                    return Err(ErrorMotor::SinCodificador(
                        "el codificador dejo de aceptar fotogramas".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// El ritmo de verdad, para los contadores (una vez por segundo).
    pub fn recontar(&mut self, ahora: i64, contadores: &Contadores) {
        let fps = self.medidor.por_segundo(ahora);
        contadores.fps.store(fps.to_bits(), Ordering::Relaxed);
    }

    /// **Lo ultimo antes de cerrar el MP4**: si no hubo ningun fotograma,
    /// uno negro que lo cubra todo (para que el MP4 valga); y el ultimo,
    /// hasta donde acaba el sonido. Devuelve ese final, en 100 ns.
    pub fn cerrar<T: Tuberia>(&mut self, tubo: &mut T, ahora: i64) -> i64 {
        let fin = self.reloj.final_(ahora);
        if tubo.escritos() == 0 {
            let _ = tubo.repetir(0, fin.max(self.duracion));
        }
        if let Err(e) = tubo.vaciar(fin, self.duracion) {
            tracing::warn!(?e, "el ultimo fotograma del video no entro");
        }
        fin
    }
}

#[cfg(test)]
mod pruebas {
    use std::cell::{Cell, RefCell};

    use super::*;

    const S: i64 = UNIDADES_POR_SEGUNDO;
    const MS: i64 = S / 1000;

    /// Un fotograma de mentira: su hora y si se solto.
    struct Foto {
        hora: i64,
        soltados: std::rc::Rc<Cell<u32>>,
    }

    impl Retenido for Foto {
        fn hora(&self) -> i64 {
            self.hora
        }
        fn soltar(self) {
            self.soltados.set(self.soltados.get() + 1);
        }
    }

    #[derive(Default)]
    struct Pantalla {
        retenido: RefCell<Option<i64>>,
        soltados: std::rc::Rc<Cell<u32>>,
    }

    impl Pantalla {
        fn llega(&self, hora: i64) {
            *self.retenido.borrow_mut() = Some(hora);
        }
    }

    impl Captura for Pantalla {
        type Retenido = Foto;
        fn hora_retenida(&self) -> Option<i64> {
            *self.retenido.borrow()
        }
        fn tomar(&self) -> Option<Foto> {
            self.retenido.borrow_mut().take().map(|hora| Foto {
                hora,
                soltados: self.soltados.clone(),
            })
        }
        fn cerrada(&self) -> bool {
            false
        }
    }

    /// Lo que la tuberia recibio.
    #[derive(Debug, Clone, PartialEq)]
    enum Dado {
        Nuevo(i64),
        Repetido(i64, i64),
        Sonido(usize, u64),
        Hasta(i64),
    }

    #[derive(Default)]
    struct Registro {
        dados: Vec<Dado>,
        lleno: bool,
        escritos: u64,
    }

    impl Tuberia for Registro {
        type Retenido = Foto;
        fn fotograma(&mut self, _: &Foto, inicio: i64, _: i64) -> Result<Escrito, ErrorMotor> {
            if self.lleno {
                return Ok(Escrito::Caido);
            }
            self.dados.push(Dado::Nuevo(inicio));
            self.escritos += 1;
            Ok(Escrito::Hecho)
        }
        fn repetir(&mut self, inicio: i64, duracion: i64) -> Result<Escrito, ErrorMotor> {
            if self.lleno {
                return Ok(Escrito::Caido);
            }
            self.dados.push(Dado::Repetido(inicio, duracion));
            self.escritos += 1;
            Ok(Escrito::Hecho)
        }
        fn audio(&mut self, muestras: &[i16], posicion: u64) -> Result<(), ErrorMotor> {
            self.dados.push(Dado::Sonido(muestras.len(), posicion));
            Ok(())
        }
        fn vaciar(&mut self, fin: i64, _: i64) -> Result<(), ErrorMotor> {
            self.dados.push(Dado::Hasta(fin));
            Ok(())
        }
        fn escritos(&self) -> u64 {
            self.escritos
        }
    }

    #[test]
    fn sin_sonido_va_el_primero_en_el_cero_y_luego_el_latido() {
        let c = Contadores::default();
        let p = Pantalla::default();
        let mut t = Registro::default();
        let mut n = Nucleo::nuevo(None, 15, 100 * S);
        p.llega(100 * S + 5 * MS);
        assert_eq!(
            n.paso(100 * S + 6 * MS, &p, &mut t, &c).unwrap(),
            Some(Escrito::Hecho)
        );
        // Nada nuevo: hasta el segundo, nada; en el segundo, el repetido.
        assert_eq!(n.paso(100 * S + 500 * MS, &p, &mut t, &c).unwrap(), None);
        n.paso(101 * S + 6 * MS, &p, &mut t, &c).unwrap();
        assert_eq!(
            t.dados,
            vec![Dado::Nuevo(0), Dado::Repetido(S + 6 * MS, S / 15)]
        );
        assert_eq!(p.soltados.get(), 1, "el fotograma tomado se solto");
        assert_eq!(c.estadisticas().fotogramas, 2);
    }

    #[test]
    fn en_pausa_no_se_escribe_nada_y_lo_retenido_se_suelta() {
        let c = Contadores::default();
        let p = Pantalla::default();
        let mut t = Registro::default();
        let mut n = Nucleo::nuevo(None, 15, 0);
        p.llega(0);
        n.paso(MS, &p, &mut t, &c).unwrap();
        n.pausa(Some(&p), true, S);
        p.llega(2 * S);
        // Caso negativo: ni el nuevo ni el latido durante la pausa.
        assert_eq!(n.paso(5 * S, &p, &mut t, &c).unwrap(), None);
        assert_eq!(p.hora_retenida(), None, "lo de la pausa se solto");
        assert_eq!(p.soltados.get(), 2);
        n.pausa(Some(&p), false, 10 * S);
        p.llega(10 * S + 100 * MS);
        n.paso(10 * S + 101 * MS, &p, &mut t, &c).unwrap();
        // Nueve segundos de pausa que no cuentan: 10,1 - 9 = 1,1 s.
        assert_eq!(t.dados.last(), Some(&Dado::Nuevo(S + 100 * MS)));
    }

    #[test]
    fn el_sonido_que_pisa_lo_escrito_se_recorta_y_ancla_el_reloj() {
        let c = Contadores::default();
        let p = Pantalla::default();
        let mut t = Registro::default();
        let mut n = Nucleo::nuevo(Some(FormatoAudio::mono(48_000)), 15, 0);
        n.audio(&mut t, &[0; 480], 0, 10 * MS).unwrap();
        // 100 muestras repetidas al principio de este bloque.
        n.audio(&mut t, &[0; 480], 380, 20 * MS).unwrap();
        // Caso negativo: uno repetido entero no se escribe.
        n.audio(&mut t, &[0; 100], 500, 20 * MS).unwrap();
        assert_eq!(t.dados, vec![Dado::Sonido(480, 0), Dado::Sonido(380, 480)]);
        // Estereo: se recorta por marcos, no por muestras sueltas.
        let mut n2 = Nucleo::nuevo(
            Some(FormatoAudio {
                muestreo: 48_000,
                canales: 2,
            }),
            15,
            0,
        );
        let mut t2 = Registro::default();
        n2.audio(&mut t2, &[0; 960], 0, 0).unwrap();
        n2.audio(&mut t2, &[0; 960], 400, 0).unwrap();
        assert_eq!(t2.dados[1], Dado::Sonido(800, 480));
        // El reloj sabe donde acaba el sonido: el primero en el cero, el
        // segundo pegado al final del sonido.
        p.llega(15 * MS);
        n.paso(16 * MS, &p, &mut t, &c).unwrap();
        p.llega(100 * MS);
        n.paso(101 * MS, &p, &mut t, &c).unwrap();
        let fin_sonido = crate::reloj::tiempo_de_muestras(860, 48_000);
        assert_eq!(t.dados.last(), Some(&Dado::Nuevo(fin_sonido + 80 * MS)));
    }

    #[test]
    fn un_codificador_atascado_diez_segundos_corta_y_una_pausa_no_cuenta() {
        let c = Contadores::default();
        let p = Pantalla::default();
        let mut t = Registro {
            lleno: true,
            ..Default::default()
        };
        let mut n = Nucleo::nuevo(None, 15, 0);
        p.llega(0);
        assert_eq!(n.paso(MS, &p, &mut t, &c).unwrap(), Some(Escrito::Caido));
        // Una pausa larga en medio: al seguir no parece un atasco.
        n.pausa(Some(&p), true, S);
        n.pausa(Some(&p), false, 60 * S);
        assert!(n.paso(61 * S, &p, &mut t, &c).is_ok());
        // Caso negativo: diez segundos seguidos sin entrar, se corta.
        let mut cortado = false;
        for i in 0..40 {
            if n.paso(61 * S + i * S / 2, &p, &mut t, &c).is_err() {
                cortado = true;
                break;
            }
        }
        assert!(cortado);
        assert!(c.estadisticas().caidos >= 2);
    }

    #[test]
    fn sin_ningun_fotograma_el_cierre_pone_uno_negro_que_lo_cubre_todo() {
        let mut t = Registro::default();
        let mut n = Nucleo::nuevo(None, 15, 0);
        let fin = n.cerrar(&mut t, 3 * S);
        assert_eq!(fin, 3 * S);
        assert_eq!(t.dados, vec![Dado::Repetido(0, 3 * S), Dado::Hasta(3 * S)]);
        // Caso negativo: con fotogramas, ningun negro.
        let mut t = Registro::default();
        let p = Pantalla::default();
        let c = Contadores::default();
        let mut n = Nucleo::nuevo(None, 15, 0);
        p.llega(0);
        n.paso(MS, &p, &mut t, &c).unwrap();
        n.cerrar(&mut t, 2 * S);
        assert_eq!(t.dados, vec![Dado::Nuevo(0), Dado::Hasta(2 * S)]);
    }

    #[test]
    fn la_espera_tiene_suelo_y_techo() {
        let n = Nucleo::nuevo(None, 15, 0);
        assert_eq!(n.espera(0, true), 10_000);
        assert_eq!(n.espera(0, false), S / 4);
    }
}
