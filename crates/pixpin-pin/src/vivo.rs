//! El pin en vivo: una zona de la pantalla vista en directo dentro de un pin.
//!
//! El pin no sabe capturar: `pixpin-capture` es su misma capa y la regla de
//! capas le prohibe verlo. Recibe una `FuenteViva` que le da, cuando hay
//! fotograma nuevo, una textura del tamano de la zona. Es la misma forma
//! que el video (D63): el pin envuelve la textura como bitmap UNA vez y en
//! cada fotograma solo repinta.
//!
//! No hay temporizador: la fuente despierta la ventana con
//! `MSG_FOTOGRAMA_VIVO` cuando la captura acepta un fotograma. Una zona
//! quieta no cuesta ni un mensaje.

use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// El mensaje con el que la captura avisa al pin de que hay fotograma.
/// Privado de la clase del pin: ninguna otra ventana lo recibe.
pub const MSG_FOTOGRAMA_VIVO: u32 = WM_APP + 0x50;

/// De donde saca un pin en vivo sus fotogramas.
pub trait FuenteViva {
    /// Si hay fotograma nuevo, la textura con la zona ya recortada. Siempre
    /// la MISMA textura: el pin la envuelve una vez y no la vuelve a pedir
    /// como bitmap.
    fn tick(&mut self) -> Option<ID3D11Texture2D>;
}
