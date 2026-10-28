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

/// **Las redes IPv4 de este equipo** (28-oct-2026): la direccion y el largo
/// del prefijo de cada adaptador encendido, de la ruta preferida (menor
/// metrica) a la ultima. Sin loopback ni las `169.254.x.x` que Windows se
/// inventa cuando el DHCP no contesta. Las VPN entran: puede que el movil
/// este al otro lado.
///
/// La baliza de la sincronizacion (`pixpin_sincro::baliza`) abre un socket
/// en cada una, porque una difusion solo sale por la red del socket que la
/// manda. Vacia si Windows no contesta: entonces se usa la de la ruta por
/// defecto.
pub fn interfaces_ipv4() -> Vec<(std::net::Ipv4Addr, u8)> {
    use windows::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_FRIENDLY_NAME,
        GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, IF_TYPE_SOFTWARE_LOOPBACK,
        IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    let banderas = GAA_FLAG_SKIP_ANYCAST
        | GAA_FLAG_SKIP_MULTICAST
        | GAA_FLAG_SKIP_DNS_SERVER
        | GAA_FLAG_SKIP_FRIENDLY_NAME;
    let mut largo: u32 = 16 * 1024;
    for _ in 0..3 {
        // En `u64` para que la lista quede alineada como la quiere Windows.
        let mut buf: Vec<u64> = vec![0; (largo as usize).div_ceil(8)];
        let lista = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `lista` apunta a `largo` bytes nuestros, alineados, que
        // viven hasta el final de esta vuelta; Windows escribe dentro y dice
        // en `largo` cuanto le hace falta si no cabe.
        let r = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_INET.0),
                banderas,
                None,
                Some(lista),
                &mut largo,
            )
        };
        if r == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if r != NO_ERROR.0 {
            tracing::info!(r, "no se pudieron leer las redes de este equipo");
            return Vec::new();
        }
        let mut redes: Vec<(u32, std::net::Ipv4Addr, u8)> = Vec::new();
        let mut a = lista.cast_const();
        while !a.is_null() {
            // SAFETY: `a` es un nodo de la lista que acaba de escribir
            // Windows dentro de `buf`, que sigue vivo; los `Next` apuntan
            // dentro del mismo `buf` o son nulos.
            let ad = unsafe { &*a };
            if ad.OperStatus == IfOperStatusUp && ad.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
                let mut u = ad.FirstUnicastAddress.cast_const();
                while !u.is_null() {
                    // SAFETY: igual que arriba, un nodo dentro de `buf`.
                    let un = unsafe { &*u };
                    let sa = un.Address.lpSockaddr;
                    // SAFETY: `lpSockaddr` apunta a una direccion de la
                    // lista, dentro de `buf`.
                    if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                        // SAFETY: con familia `AF_INET` es un `SOCKADDR_IN`.
                        let b = unsafe { (*sa.cast::<SOCKADDR_IN>()).sin_addr.S_un.S_un_b };
                        let ip = std::net::Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4);
                        if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() {
                            redes.push((ad.Ipv4Metric, ip, un.OnLinkPrefixLength.min(32)));
                        }
                    }
                    u = un.Next.cast_const();
                }
            }
            a = ad.Next.cast_const();
        }
        redes.sort_by_key(|(metrica, ..)| *metrica);
        return redes.into_iter().map(|(_, ip, p)| (ip, p)).collect();
    }
    Vec::new()
}
