//! **Un solo trabajo gordo de memoria a la vez, en modo cuidadoso.**
//!
//! Whisper sin arena pica en ~350 MB y pdfsqueeze en modo cuidadoso en
//! ~339 MB (medidos). En un equipo de 4 GB, los dos a la vez —o dos notas
//! transcribiendose a la vez— se comen lo que le queda a Windows. Asi que en
//! modo cuidadoso los dos piden **turno** antes de empezar: el segundo espera
//! a que acabe el primero (una cola, por orden de llegada al cerrojo). Fuera
//! del modo cuidadoso el turno no espera a nadie: hay memoria de sobra.
//!
//! **La regla del modo cuidadoso es la de `crate::aligerar::modo_cuidadoso`**
//! (4 GB o menos, o nivel Ligero): no se duplica, se usa esa.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

static CUIDADOSO: AtomicBool = AtomicBool::new(false);
static TURNO: Mutex<()> = Mutex::new(());

/// **Decide el modo del equipo** al arrancar, con los hechos del equipo:
/// lo apunta aqui, se lo dice a Whisper y deja en el registro que se eligio
/// y por que.
pub fn fijar_equipo(ram_fisica_bytes: u64, nivel_ligero: bool) {
    let c = crate::aligerar::modo_cuidadoso(ram_fisica_bytes, nivel_ligero);
    CUIDADOSO.store(c, Ordering::Relaxed);
    pixpin_voz::whisper::fijar_modo_cuidadoso(c);
    let r = reparto(ram_fisica_bytes, nivel_ligero, nucleos());
    let razon = if !c {
        "memoria de sobra"
    } else if nivel_ligero {
        "nivel de rendimiento Ligero"
    } else {
        "4 GB de RAM o menos"
    };
    tracing::info!(
        cuidadoso = c,
        ram_mb = ram_fisica_bytes / (1024 * 1024),
        nivel_ligero,
        hilos = r.hilos,
        arena = r.arena,
        razon,
        "Whisper: modo del equipo (transcripciones de una en una y sin coincidir con aligerar PDF si es cuidadoso)"
    );
}

fn nucleos() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Como correra Whisper en un equipo con estos hechos (pura, para probarla).
pub fn reparto(
    ram_fisica_bytes: u64,
    nivel_ligero: bool,
    nucleos: usize,
) -> pixpin_voz::whisper::Reparto {
    pixpin_voz::whisper::reparto(
        crate::aligerar::modo_cuidadoso(ram_fisica_bytes, nivel_ligero),
        nucleos,
    )
}

/// Si el equipo va en modo cuidadoso.
pub fn cuidadoso() -> bool {
    CUIDADOSO.load(Ordering::Relaxed)
}

/// El turno, mientras se tenga. Soltarlo es dejarlo caer.
pub struct Turno(#[allow(dead_code)] Option<MutexGuard<'static, ()>>);

/// **Pide turno** para un trabajo gordo (`quien`, para el registro). En
/// modo cuidadoso **espera** a que acabe el que lo tenga; si no, vuelve en
/// el acto. Nunca desde el hilo de una ventana.
pub fn tomar(quien: &str) -> Turno {
    if !cuidadoso() {
        return Turno(None);
    }
    let guardia = match TURNO.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::info!(quien, "modo cuidadoso: espera turno de memoria");
            // Un hilo que murio con el turno no lo deja cogido para siempre.
            TURNO.lock().unwrap_or_else(|e| e.into_inner())
        }
    };
    Turno(Some(guardia))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn con_cuatro_gigas_whisper_va_sin_arena_y_con_pocos_hilos() {
        let r = reparto(4 * GIB, false, 8);
        assert!(!r.arena);
        assert!(r.hilos <= 2);
    }

    #[test]
    fn con_dieciseis_gigas_y_nivel_completo_whisper_va_con_arena() {
        let r = reparto(16 * GIB, false, 8);
        assert!(r.arena);
        assert_eq!(r.hilos, 8);
    }

    #[test]
    fn con_nivel_ligero_aunque_haya_ocho_gigas_va_sin_arena() {
        let r = reparto(8 * GIB, true, 8);
        assert!(!r.arena);
        assert!(r.hilos <= 2);
    }

    #[test]
    fn en_modo_cuidadoso_el_segundo_trabajo_espera_al_primero_y_fuera_no() {
        // Fuera del modo cuidadoso no se espera a nadie.
        CUIDADOSO.store(false, Ordering::Relaxed);
        let a = tomar("a");
        let b = tomar("b");
        drop((a, b));

        CUIDADOSO.store(true, Ordering::Relaxed);
        let primero = tomar("whisper");
        let (tx, rx) = std::sync::mpsc::channel();
        let h = std::thread::spawn(move || {
            let _t = tomar("pdf");
            tx.send(()).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(200))
                .is_err(),
            "el segundo no empieza mientras el primero tiene el turno"
        );
        drop(primero);
        assert!(rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok());
        h.join().unwrap();
        CUIDADOSO.store(false, Ordering::Relaxed);
    }
}
