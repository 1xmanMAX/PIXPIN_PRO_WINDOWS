//! **El permiso de red de PinDesk en el cortafuegos de Windows** (3-nov-2026).
//!
//! La sincronizacion escucha en TCP 47474 y en UDP 47475 (la baliza). Para
//! que el movil pueda llamar, el cortafuegos tiene que dejar entrar a
//! `pindesk.exe`. Normalmente lo pregunta Windows la primera vez que la app
//! escucha y la regla que crea al pulsar «Permitir» es para el programa. Pero
//! la regla va **por ruta**: tras pasar de `Programs\PixPinMax\pixpinmax.exe`
//! a `Programs\PinDesk\pindesk.exe`, la de antes no vale, y si el aviso nuevo
//! se cerro sin mirar (sale detras de la consola del instalador, o al
//! arrancar con Windows) Windows deja una regla de **bloqueo** para el exe
//! nuevo, que gana a cualquier permiso. Sin decir nada: el movil no llega.
//!
//! - [`permiso`]: si el cortafuegos deja entrar a este programa en un puerto,
//!   con el perfil de red de ahora (`INetFwMgr::IsPortAllowed`, que tiene en
//!   cuenta reglas de programa y de puerto, bloqueos y si esta apagado).
//! - [`pedir_permiso`]: pone las reglas con PowerShell **como
//!   administrador** (el aviso de Control de cuentas de usuario lo explica la
//!   ventana antes de lanzarlo): quita los bloqueos de este programa y las
//!   reglas de antes, y deja entrar TCP y UDP 47475 en redes privadas y de
//!   dominio. Es lo mismo que `herramientas/cortafuegos-sincro.ps1`.

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, WAIT_OBJECT_0};
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwMgr, NET_FW_IP_PROTOCOL_TCP, NET_FW_IP_PROTOCOL_UDP, NET_FW_IP_VERSION_V4, NetFwMgr,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::System::Variant::{VARIANT, VariantClear, VariantToBooleanWithDefault};
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{BSTR, HSTRING, PCWSTR, w};

/// El nombre de las reglas que pone PinDesk (y el de antes, que se quita).
pub const NOMBRE_REGLA: &str = "PinDesk - Sincronizar";
const NOMBRE_REGLA_ANTES: &str = "PixPin Max - Sincronizar";

/// Lo que dice el cortafuegos de un puerto de este programa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permiso {
    /// Entra (con una regla, o el cortafuegos esta apagado en esta red).
    Permitido,
    /// No entra: sin regla o con una de bloqueo.
    Bloqueado,
    /// Windows no contesto (servicio parado, directiva): no se sabe.
    NoSe,
}

/// **Si el cortafuegos deja entrar a `programa` en `puerto`** (TCP o UDP)
/// con el perfil de red de ahora. Inicia COM en este hilo si hace falta.
pub fn permiso(programa: &Path, puerto: u16, udp: bool) -> Permiso {
    let _com = crate::explorador::ComDelHilo::iniciar();
    // SAFETY: COM esta iniciado en este hilo (o ya lo estaba); la interfaz y
    // los BSTR viven hasta el final de la funcion y los VARIANT se limpian.
    unsafe {
        let Ok(mgr) = CoCreateInstance::<_, INetFwMgr>(&NetFwMgr, None, CLSCTX_INPROC_SERVER)
        else {
            return Permiso::NoSe;
        };
        let ruta = BSTR::from(programa.as_os_str().to_string_lossy().as_ref());
        let mut permitido = VARIANT::default();
        let mut restringido = VARIANT::default();
        let r = mgr.IsPortAllowed(
            &ruta,
            NET_FW_IP_VERSION_V4,
            i32::from(puerto),
            &BSTR::new(),
            if udp {
                NET_FW_IP_PROTOCOL_UDP
            } else {
                NET_FW_IP_PROTOCOL_TCP
            },
            &mut permitido,
            &mut restringido,
        );
        let si = VariantToBooleanWithDefault(&permitido, false).as_bool();
        let _ = VariantClear(&mut permitido);
        let _ = VariantClear(&mut restringido);
        match r {
            Ok(()) if si => Permiso::Permitido,
            Ok(()) => Permiso::Bloqueado,
            Err(e) => {
                tracing::info!(?e, "cortafuegos: IsPortAllowed no contesto");
                Permiso::NoSe
            }
        }
    }
}

/// Como acabo [`pedir_permiso`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pedido {
    /// Las reglas quedaron puestas.
    Hecho,
    /// El usuario dijo «No» en el aviso de administrador.
    Cancelado,
    /// PowerShell no pudo (directiva de empresa, sin servicio).
    Fallo,
}

/// **Pone las reglas como administrador** y espera a que acabe (unos
/// segundos; llamar desde un hilo aparte). Windows ensena su aviso de
/// Control de cuentas de usuario.
pub fn pedir_permiso(programa: &Path) -> Pedido {
    let parametros = HSTRING::from(format!(
        "-NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {}",
        orden_codificada(&orden(programa))
    ));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: w!("powershell.exe"),
        lpParameters: PCWSTR(parametros.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: la estructura y las cadenas viven durante la llamada; el
    // proceso que devuelve se espera y se cierra aqui mismo.
    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            return if e.code() == ERROR_CANCELLED.to_hresult() {
                Pedido::Cancelado
            } else {
                tracing::warn!(?e, "cortafuegos: no se pudo lanzar PowerShell");
                Pedido::Fallo
            };
        }
        if info.hProcess.is_invalid() {
            return Pedido::Fallo;
        }
        let espera = WaitForSingleObject(info.hProcess, 60_000);
        let mut codigo = 1u32;
        let bien = espera == WAIT_OBJECT_0
            && GetExitCodeProcess(info.hProcess, &mut codigo).is_ok()
            && codigo == 0;
        let _ = CloseHandle(info.hProcess);
        if bien {
            Pedido::Hecho
        } else {
            tracing::warn!(codigo, "cortafuegos: PowerShell no pudo poner las reglas");
            Pedido::Fallo
        }
    }
}

/// **La orden de PowerShell** que pone las reglas de `programa`. Pura.
///
/// 1. Quita las reglas de **bloqueo** de entrada de este programa: las crea
///    Windows al cerrar su aviso sin permitir, y un bloqueo gana a cualquier
///    permiso.
/// 2. Quita las de PinDesk (y las de PixPin Max) puestas antes.
/// 3. Deja entrar TCP (el puerto puede no ser el 47474 si estaba ocupado) y
///    UDP 47475 (la baliza), en redes privadas y de dominio. En una red
///    «Pública» no: se le dice al usuario que la marque como privada.
pub fn orden(programa: &Path) -> String {
    let ruta = programa.display().to_string().replace('\'', "''");
    format!(
        "$ErrorActionPreference = 'Stop'\n\
         $p = '{ruta}'\n\
         Get-NetFirewallApplicationFilter -Program $p -ErrorAction SilentlyContinue | \
         Get-NetFirewallRule | Where-Object {{ $_.Direction -eq 'Inbound' -and $_.Action -eq 'Block' }} | \
         Remove-NetFirewallRule\n\
         foreach ($n in '{NOMBRE_REGLA}', '{NOMBRE_REGLA_ANTES}') {{ \
         Get-NetFirewallRule -DisplayName \"$n*\" -ErrorAction SilentlyContinue | Remove-NetFirewallRule }}\n\
         New-NetFirewallRule -DisplayName '{NOMBRE_REGLA} (TCP)' -Direction Inbound -Action Allow \
         -Program $p -Protocol TCP -Profile Private,Domain | Out-Null\n\
         New-NetFirewallRule -DisplayName '{NOMBRE_REGLA} (baliza UDP 47475)' -Direction Inbound \
         -Action Allow -Program $p -Protocol UDP -LocalPort 47475 -Profile Private,Domain | Out-Null\n\
         exit 0\n"
    )
}

/// `-EncodedCommand`: la orden en UTF-16LE y en base64. Asi una ruta con
/// espacios, comillas o acentos llega entera, sin pelear con las comillas.
pub fn orden_codificada(orden: &str) -> String {
    let bytes: Vec<u8> = orden.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64(&bytes)
}

fn base64(datos: &[u8]) -> String {
    const SIGNOS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(datos.len().div_ceil(3) * 4);
    for trozo in datos.chunks(3) {
        let b = [
            trozo[0],
            trozo.get(1).copied().unwrap_or(0),
            trozo.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= trozo.len() {
                s.push(SIGNOS[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_base64_es_el_de_libro() {
        // RFC 4648 §10.
        for (entrada, salida) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(entrada.as_bytes()), salida);
        }
        // PowerShell espera UTF-16LE: «a» son dos bytes, 61 00.
        assert_eq!(orden_codificada("a"), "YQA=");
    }

    #[test]
    fn la_orden_quita_los_bloqueos_y_deja_entrar_tcp_y_la_baliza() {
        let o = orden(Path::new(
            r"C:\Users\Ana O'Neil\AppData\Local\Programs\PinDesk\pindesk.exe",
        ));
        assert!(
            o.contains(r"$p = 'C:\Users\Ana O''Neil\AppData\Local\Programs\PinDesk\pindesk.exe'")
        );
        assert!(o.contains("$_.Action -eq 'Block'"), "los bloqueos primero");
        assert!(o.contains("-Protocol TCP -Profile Private,Domain"));
        assert!(o.contains("-Protocol UDP -LocalPort 47475 -Profile Private,Domain"));
        assert!(o.contains("'PixPin Max - Sincronizar'"), "y las de antes");
        // Caso negativo: nunca abre la red publica.
        assert!(!o.contains("Public"));
    }
}
