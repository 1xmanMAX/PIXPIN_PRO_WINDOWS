//! **Saber cuando cambia la red** (27-oct-2026): Windows avisa cuando una
//! direccion IPv4 de este equipo aparece o se va (`NotifyAddrChange`), que es
//! lo que pasa al reconectarse la Wi-Fi o enchufar el cable.
//!
//! Lo usa la sincronizacion automatica (`apps/pixpin/src/sincronizar/
//! automatica.rs`) para sondear al grupo en cuanto vuelve la red, como el
//! `registerDefaultNetworkCallback` del movil, en vez de esperar al siguiente
//! latido de la sonda. Aqui y no en la aplicacion porque la aplicacion
//! prohibe `unsafe`.
//!
//! IPv4 basta: la sincronizacion va por la red local, y alli las direcciones
//! que se anuncian y se apuntan son IPv4.

use windows::Win32::Foundation::NO_ERROR;
use windows::Win32::NetworkManagement::IpHelper::NotifyAddrChange;

/// Llama a `aviso` cada vez que cambia una direccion de red de este equipo,
/// desde un hilo propio que vive lo que la aplicacion. Varios cambios
/// seguidos (quitar una y poner otra) son varios avisos: quien escucha ya
/// sabe juntarlos. `false` si no se pudo lanzar el hilo.
pub fn al_cambiar(aviso: impl Fn() + Send + 'static) -> bool {
    std::thread::Builder::new()
        .name("vigia-red".into())
        .spawn(move || {
            loop {
                // SAFETY: sin HANDLE ni OVERLAPPED la llamada es sincrona:
                // bloquea este hilo hasta el proximo cambio y no se queda con
                // ningun puntero nuestro.
                let r = unsafe { NotifyAddrChange(std::ptr::null_mut(), std::ptr::null()) };
                if r != NO_ERROR.0 {
                    // Sin servicio de red (o sin permiso): no se insiste, que
                    // un fallo que vuelve enseguida seria un bucle en vacio.
                    tracing::info!(r, "no se puede vigilar la red");
                    return;
                }
                aviso();
            }
        })
        .is_ok()
}
