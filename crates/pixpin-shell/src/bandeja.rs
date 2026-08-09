//! Icono de la bandeja del sistema y su menu contextual.
//!
//! El icono es la unica presencia visible de PixPin Max cuando no estas
//! capturando. Se retira en `Drop` para que cerrar la aplicacion no deje un
//! icono fantasma que solo desaparece al pasar el raton por encima.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION, LoadIconW,
    MF_SEPARATOR, MF_STRING, SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu,
};
use windows::core::{HSTRING, Result as WinResult};

use crate::ventana::{ID_MENU_AJUSTES, ID_MENU_CAPTURAR, ID_MENU_SALIR, WM_BANDEJA};

/// Identificador del icono dentro de nuestra propia ventana. Solo hay uno.
const ID_ICONO: u32 = 1;

/// Textos del menu, ya traducidos por el catalogo Fluent.
pub struct EtiquetasMenu {
    pub capturar: String,
    pub ajustes: String,
    pub salir: String,
}

pub struct Bandeja {
    datos: NOTIFYICONDATAW,
}

impl Bandeja {
    pub fn nueva(hwnd: HWND, titulo: &str) -> WinResult<Self> {
        // SAFETY: IDI_APPLICATION es un icono del sistema siempre disponible;
        // pasar None como instancia indica que es predefinido.
        let icono = unsafe { LoadIconW(None, IDI_APPLICATION)? };

        let mut datos = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: ID_ICONO,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_BANDEJA,
            hIcon: icono,
            ..Default::default()
        };

        // szTip es un array de 128 u16 terminado en cero (Default lo deja a
        // cero entero, asi que basta con no escribir en la ultima posicion
        // para garantizar el terminador). Se copian caracteres completos, no
        // unidades UTF-16 sueltas: cortar a mitad de una pareja subrogada
        // dejaria un `char` partido justo antes del cero final, que aunque
        // seguiria terminando en NUL formaria una secuencia UTF-16 invalida.
        let mut escritos = 0usize;
        'copia: for caracter in titulo.chars() {
            let mut buf = [0u16; 2];
            for unidad in caracter.encode_utf16(&mut buf) {
                if escritos >= 127 {
                    break 'copia;
                }
                datos.szTip[escritos] = *unidad;
                escritos += 1;
            }
        }

        // SAFETY: `datos` esta completamente inicializada, su cbSize es
        // correcto y hWnd es una ventana valida de este proceso.
        unsafe {
            Shell_NotifyIconW(NIM_ADD, &datos).ok()?;
        }

        Ok(Self { datos })
    }

    /// Muestra el menu contextual donde este el raton.
    pub fn mostrar_menu(&self, hwnd: HWND, etiquetas: &EtiquetasMenu) -> WinResult<()> {
        // SAFETY: no se pasan argumentos que dependan de memoria ajena; crea
        // un menu nuevo del que esta funcion es responsable de destruir.
        let menu = unsafe { CreatePopupMenu()? };

        // A partir de aqui el menu ya existe y debe destruirse siempre, tanto
        // si el resto de la funcion tiene exito como si no. Por eso los pasos
        // intermedios devuelven su resultado a traves de esta clausura en vez
        // de propagar el error con `?` directamente desde el cuerpo de la
        // funcion, que saltaria la llamada a DestroyMenu de mas abajo: es el
        // mismo defecto que la revision de la Tarea 6 encontro en appdata().
        let resultado = (|| -> WinResult<()> {
            // SAFETY: `menu` es el recien creado arriba y sigue vivo durante
            // todo este cierre; las cadenas se pasan como HSTRING, que
            // gestiona su propia memoria durante la llamada.
            unsafe {
                AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_MENU_CAPTURAR as usize,
                    &HSTRING::from(etiquetas.capturar.as_str()),
                )?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_MENU_AJUSTES as usize,
                    &HSTRING::from(etiquetas.ajustes.as_str()),
                )?;
                AppendMenuW(menu, MF_SEPARATOR, 0, None)?;
                AppendMenuW(
                    menu,
                    MF_STRING,
                    ID_MENU_SALIR as usize,
                    &HSTRING::from(etiquetas.salir.as_str()),
                )?;
            }

            let mut punto = POINT::default();
            // SAFETY: `punto` es una estructura propia y valida a la que
            // escribir.
            unsafe {
                GetCursorPos(&mut punto)?;
            }

            // Sin esta llamada el menu no se cierra al hacer clic fuera. Es
            // un requisito documentado de TrackPopupMenu que se olvida a
            // menudo, y tiene que ir antes de TrackPopupMenu para que surta
            // efecto.
            // SAFETY: `hwnd` es la ventana propia, valida mientras dure la
            // llamada.
            let _ = unsafe { SetForegroundWindow(hwnd) };

            // SAFETY: `menu` sigue vivo, `hwnd` es la ventana propia y
            // `punto` ya se ha inicializado con GetCursorPos.
            let _ = unsafe {
                TrackPopupMenu(menu, TPM_RIGHTBUTTON, punto.x, punto.y, None, hwnd, None)
            };

            Ok(())
        })();

        // SAFETY: `menu` viene de CreatePopupMenu de mas arriba y todavia no
        // se ha destruido; se destruye aqui exactamente una vez, tanto si el
        // cierre anterior tuvo exito como si fallo a mitad, para no dejar un
        // handle de menu fugado.
        unsafe {
            let _ = DestroyMenu(menu);
        }

        resultado
    }
}

impl Drop for Bandeja {
    fn drop(&mut self) {
        // SAFETY: `datos` describe un icono añadido por nosotros con NIM_ADD y
        // aun no retirado; este tipo no es Clone ni Copy.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &self.datos);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ventana::VentanaMensajes;

    #[test]
    fn se_anade_y_se_retira_el_icono() {
        let v = VentanaMensajes::nueva().unwrap();
        let b = Bandeja::nueva(v.handle(), "PixPin Max — prueba").expect("deberia añadirse");
        drop(b);

        // Poder añadir un segundo icono tras retirar el primero demuestra que
        // el Drop hizo su trabajo y no quedo un fantasma en la bandeja.
        let otra = Bandeja::nueva(v.handle(), "PixPin Max — prueba 2").expect("y otra vez");
        drop(otra);
    }
}
