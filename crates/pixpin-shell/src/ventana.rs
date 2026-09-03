//! La ventana invisible que recibe todo y el bucle que duerme.
//!
//! Es una ventana `HWND_MESSAGE`: no se dibuja, no sale en la barra de tareas
//! y no aparece con Alt+Tab, pero recibe mensajes. Es donde llegan `WM_HOTKEY`
//! y las notificaciones de la bandeja.
//!
//! **El bucle usa `GetMessageW`, que bloquea el hilo hasta que llega algo.**
//! Esa eleccion es la que cumple el objetivo de 0% de CPU en reposo del
//! presupuesto de rendimiento. Un bucle con `PeekMessageW` giraria sin parar y
//! se comeria la bateria de un portatil sin hacer nada.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostQuitMessage, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_HOTKEY, WNDCLASSEXW,
};
use windows::core::{Result as WinResult, w};

/// Mensaje propio para las notificaciones del icono de bandeja.
pub const WM_BANDEJA: u32 = WM_APP + 1;

/// Mensaje sin contenido cuyo unico fin es hacer girar el bucle principal.
///
/// Las ventanas de los pines tienen su propio WndProc y no producen eventos
/// de esta ventana, asi que lo que dejan pendiente (una peticion del menu,
/// un Ctrl+C) no se atenderia hasta que el usuario pulsara un atajo. Con
/// esto, el pin avisa y el bucle vacia su bandeja de entrada al momento.
pub const WM_DESPERTAR: u32 = WM_APP + 2;

/// Pide al bucle principal que de una vuelta. Seguro desde cualquier hilo.
pub fn despertar(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    // SAFETY: publicar un mensaje propio en la cola de una ventana propia;
    // PostMessageW no bloquea ni toca memoria del llamante.
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_DESPERTAR, WPARAM(0), LPARAM(0));
    }
}

/// Mensaje propio del gancho de raton (`gestos.rs`): Alt + boton pulsado
/// en cualquier sitio de la pantalla. wParam lleva el boton, lParam el
/// punto en pixeles fisicos (x en la palabra baja, y en la alta).
pub const WM_GESTO: u32 = WM_APP + 3;

/// El boton con el que arranco un gesto con Alt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotonGesto {
    /// Alt + izquierdo: seleccionar y copiar directo.
    Izquierdo,
    /// Alt + derecho: seleccionar y pinear directo.
    Derecho,
}

/// Primer identificador de la seccion «Grupos ocultos»: al elegir uno, el
/// numero de grupo sale de restar esta base (spec 4.3). Por debajo viven los
/// identificadores de los comandos, que los reparte el catalogo.
pub const ID_MENU_GRUPO_BASE: u32 = 200;

/// Lo que le puede pasar a la aplicacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// Se pulso un atajo global. El numero es el identificador con el que se
    /// registro (ver `atajos.rs`).
    Atajo(u32),
    /// Se eligio una entrada del menu de la bandeja. El numero es el mismo
    /// identificador de comando que viaja en `Atajo`: una funcion, un
    /// numero, dos vias de llegada.
    Menu(u32),
    /// Se eligio un grupo oculto en la bandeja: vuelve a la pantalla (D24).
    MostrarGrupo(u32),
    /// Un pin dejo algo pendiente y pide una vuelta del bucle. No hay nada
    /// que hacer con el evento en si: el trabajo va tras el match.
    Despertar,
    /// Clic izquierdo en el icono de la bandeja.
    IconoPulsado,
    /// Alt + boton pulsado en la pantalla (gancho de raton): abrir la
    /// captura ya con el arrastre en marcha desde `punto` (fisico).
    Gesto {
        boton: BotonGesto,
        punto: pixpin_geom::Punto,
    },
}

/// Que hacer despues de atender un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Continuar {
    Si,
    No,
}

thread_local! {
    /// Cola de eventos traducidos por el `WndProc`.
    ///
    /// Se usa un `thread_local` en vez de guardar un puntero al callback en
    /// los datos de la ventana porque el `WndProc` es una funcion `extern
    /// "system"` que no puede capturar entorno, y porque toda la interaccion
    /// con ventanas ocurre en el hilo de interfaz por exigencia de Win32.
    static PENDIENTES: RefCell<Vec<Evento>> = const { RefCell::new(Vec::new()) };
}

pub struct VentanaMensajes {
    hwnd: HWND,
}

impl VentanaMensajes {
    pub fn nueva() -> WinResult<Self> {
        const CLASE: windows::core::PCWSTR = w!("PixPinMaxVentanaMensajes");

        // SAFETY: GetModuleHandleW(None) devuelve el modulo del proceso
        // actual, que siempre existe mientras el proceso vive.
        let instancia = unsafe { GetModuleHandleW(None)? };

        let clase = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(procedimiento),
            hInstance: instancia.into(),
            lpszClassName: CLASE,
            ..Default::default()
        };

        // SAFETY: `clase` esta completamente inicializada y su `lpszClassName`
        // apunta a un literal estatico. Registrar la misma clase dos veces
        // devuelve 0 y pone ERROR_CLASS_ALREADY_EXISTS, que aqui es benigno:
        // significa que otra VentanaMensajes ya la registro. Si el fallo
        // fuera por otra razon, CreateWindowExW fallara a continuacion (la
        // clase no habria quedado registrada) y ese `?` es el que reporta el
        // error real; ningun fallo se pierde en silencio.
        unsafe {
            let _ = RegisterClassExW(&clase);
        }

        // SAFETY: la clase esta registrada (por esta llamada o por una
        // anterior) y HWND_MESSAGE es el padre valido para una ventana
        // solo-mensajes.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASE,
                w!("PixPin Max"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instancia.into()),
                None,
            )?
        };

        Ok(Self { hwnd })
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    /// Bucle principal. Bloquea el hilo hasta que `al_recibir` devuelve
    /// `Continuar::No` o llega `WM_QUIT`.
    ///
    /// Toma `&self`, no `self`: si consumiera la ventana, este metodo la
    /// destruiria (via su `Drop`) en cuanto el bucle terminara, es decir
    /// antes de que `main` recupere el control. `bandeja` y los atajos
    /// registrados, que en `main` se declaran despues de la ventana y por
    /// tanto se sueltan antes en el orden inverso normal de Rust, se
    /// habrian soltado entonces contra un `HWND` ya destruido. Con `&self`
    /// la ventana sigue viva hasta que `main` termina de verdad, y el orden
    /// de caida natural (atajos, luego bandeja, luego ventana) es el
    /// correcto.
    pub fn ejecutar(&self, mut al_recibir: impl FnMut(Evento) -> Continuar) {
        let mut mensaje = MSG::default();
        loop {
            // SAFETY: `mensaje` es una estructura propia y valida.
            // GetMessageW devuelve >0 si hay mensaje, 0 al recibir WM_QUIT y
            // -1 si hay error; los dos ultimos casos se tratan igual aqui
            // porque `ejecutar` no devuelve Result y no hay nada mas que
            // hacer salvo dejar de bloquear el hilo.
            let resultado = unsafe { GetMessageW(&mut mensaje, None, 0, 0) };
            if resultado.0 <= 0 {
                break;
            }

            // SAFETY: `mensaje` viene de GetMessageW y es valido.
            unsafe {
                let _ = TranslateMessage(&mensaje);
                DispatchMessageW(&mensaje);
            }

            // Se drena en bucle, no de una sola vez. `DispatchMessageW` de
            // arriba puede haber ejecutado codigo que a su vez entra en un
            // bucle de mensajes anidado (TrackPopupMenu, en
            // bandeja.rs::mostrar_menu, es exactamente eso), y ese bucle
            // anidado puede repartir un WM_COMMAND que el WndProc encola en
            // PENDIENTES mientras el `al_recibir` de mas abajo todavia esta
            // en marcha para el evento que abrio el menu. Con un solo drain
            // ese evento quedaria en la cola hasta el siguiente mensaje de
            // Win32 -- que con `GetMessageW` puede tardar en llegar -- y la
            // aplicacion parece ignorar el clic. Volver a drenar aqui,
            // sin pasar otra vez por `GetMessageW`, lo recoge de inmediato.
            loop {
                let eventos: Vec<Evento> = PENDIENTES.with(|p| p.borrow_mut().drain(..).collect());
                if eventos.is_empty() {
                    break;
                }
                for evento in eventos {
                    if al_recibir(evento) == Continuar::No {
                        return;
                    }
                }
            }
        }
    }
}

impl Drop for VentanaMensajes {
    fn drop(&mut self) {
        // SAFETY: `hwnd` viene de CreateWindowExW y no se ha destruido antes;
        // este tipo no es Clone ni Copy, asi que se destruye exactamente una
        // vez.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Traduce mensajes de Win32 a `Evento` y los deja en la cola del hilo.
///
/// No hace trabajo real: cuanto antes vuelva, mas fluido va todo lo demas.
extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{WM_COMMAND, WM_DESTROY, WM_LBUTTONUP};

    let evento = match mensaje {
        WM_HOTKEY => Some(Evento::Atajo(wparam.0 as u32)),
        WM_COMMAND => match (wparam.0 & 0xFFFF) as u32 {
            c if c >= ID_MENU_GRUPO_BASE => Some(Evento::MostrarGrupo(c - ID_MENU_GRUPO_BASE)),
            0 => None,
            c => Some(Evento::Menu(c)),
        },
        // Esta comparacion asume la semantica "clasica" de Shell_NotifyIconW,
        // donde lParam es directamente el mensaje del raton (WM_LBUTTONUP,
        // etc). Si en el futuro el icono de la bandeja se registra con
        // NOTIFYICON_VERSION_4 (via Shell_NotifyIconW + NIM_SETVERSION),
        // lParam pasa a llevar las coordenadas del cursor empaquetadas en la
        // palabra baja/alta en vez del mensaje, y esta igualdad exacta deja
        // de reconocer el clic sin dar ningun error visible. Quien conecte
        // la bandeja (tarea futura) debe revisar esto si cambia la version
        // notificada.
        WM_DESPERTAR => Some(Evento::Despertar),
        WM_GESTO => {
            let boton = if wparam.0 == 0 {
                BotonGesto::Izquierdo
            } else {
                BotonGesto::Derecho
            };
            // Las coordenadas van en dos palabras de 16 bits con signo: un
            // monitor a la izquierda del principal tiene x negativa.
            let x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
            Some(Evento::Gesto {
                boton,
                punto: pixpin_geom::Punto { x, y },
            })
        }
        WM_BANDEJA if (lparam.0 as u32) == WM_LBUTTONUP => Some(Evento::IconoPulsado),
        WM_DESTROY => {
            // SAFETY: llamada sin argumentos que solo encola WM_QUIT en la
            // cola de mensajes de este hilo; no toca memoria ajena.
            unsafe { PostQuitMessage(0) };
            None
        }
        _ => None,
    };

    if let Some(evento) = evento {
        PENDIENTES.with(|p| p.borrow_mut().push(evento));
        return LRESULT(0);
    }

    // SAFETY: delegar en el procedimiento por defecto siempre es valido para
    // los mensajes que no tratamos; hwnd, mensaje, wparam y lparam son los
    // que Windows paso a este WndProc.
    unsafe { DefWindowProcW(hwnd, mensaje, wparam, lparam) }
}

/// Saca de la cola los atajos globales pendientes y devuelve sus ids.
///
/// Lo usa el bucle modal del overlay: mientras un overlay esta abierto, un
/// atajo pulsado NO puede quedarse esperando en esta cola, porque al cerrar
/// el overlay se atenderia y lo volveria a abrir. En vez de eso se le
/// entrega al overlay, que decide (la capa viva alterna el modo pasante,
/// D50; el overlay de captura lo ignora).
pub(crate) fn tomar_atajos_pendientes() -> Vec<u32> {
    PENDIENTES.with(|p| {
        let mut cola = p.borrow_mut();
        let mut ids = Vec::new();
        cola.retain(|e| match e {
            Evento::Atajo(id) => {
                ids.push(*id);
                false
            }
            _ => true,
        });
        ids
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::IsWindow;

    #[test]
    fn se_crea_y_se_destruye_sin_fugas() {
        let v = VentanaMensajes::nueva().expect("deberia poder crearse");
        let hwnd = v.handle();
        assert!(!hwnd.is_invalid(), "el handle no puede ser invalido");

        // SAFETY: `hwnd` viene de CreateWindowExW y sigue vivo (v todavia no
        // se ha soltado). Consultar con IsWindow un handle vivo es siempre
        // valido y no toma posesion de nada.
        let sigue_viva = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            sigue_viva,
            "la ventana debe existir para Windows mientras v esta viva"
        );

        drop(v);

        // Este es el aserto que hace real el nombre del test: comprueba que
        // Drop destruyo de verdad la ventana, no solo que el handle guardado
        // dejo de ser invalido (eso ya lo era desde el principio y no lo
        // cambia soltar `v`). Sin esto, un Drop que nunca llamara a
        // DestroyWindow pasaria el test igual.
        //
        // SAFETY: `hwnd` es el valor numerico de un handle que ya se destruyo;
        // consultar un handle destruido con IsWindow es una operacion valida
        // que precisamente sirve para comprobar que ya no existe, no un uso
        // despues de liberar memoria (IsWindow no dereferencia el puntero,
        // solo busca la entrada en la tabla de ventanas del sistema).
        let sigue_reconocida = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            !sigue_reconocida,
            "tras soltar v, Windows ya no debe reconocer el handle"
        );

        // Crear una segunda tras destruir la primera comprueba que la clase de
        // ventana se registra de forma reentrante y que no queda basura.
        let otra = VentanaMensajes::nueva().expect("la segunda tambien");
        assert!(!otra.handle().is_invalid());
    }

    /// Comprueba la PRECONDICION de la que depende el arreglo del hallazgo
    /// 2, no el arreglo en si.
    ///
    /// Importante, para no prometer mas de lo que da: este test **no llama
    /// a `ejecutar`**. Suelta `registrados`, `bandeja` y `ventana` a mano,
    /// en el mismo orden en que `main()` los declara. Con eso comprueba,
    /// via `IsWindow` en cada paso de la caida, que ese orden de
    /// declaracion por si solo -- sin que nada mueva `ventana` fuera de su
    /// sitio -- ya basta para que Rust suelte hotkeys -> bandeja -> ventana
    /// (la ventana sigue viva cuando `Bandeja` y `AtajosRegistrados` se
    /// sueltan, y solo deja de existir cuando `VentanaMensajes` se suelta al
    /// final). Pasaria igual con la firma antigua `ejecutar(self, ...)`,
    /// porque esa firma no interviene aqui en absoluto.
    ///
    /// Lo que de verdad arreglo el hallazgo 2 -- que `ejecutar` tomaba
    /// `self` por valor, asi que devolver del bucle destruia la ventana
    /// *dentro* de esa llamada, antes de que `main` recuperase el control,
    /// y `bandeja`/`atajos` se soltaban entonces contra un `HWND` ya muerto
    /// -- es la firma `pub fn ejecutar(&self, ...)` de mas arriba en este
    /// mismo fichero; no hay un test que ejercite ese camino exacto (haria
    /// falta lanzar el bucle real y cerrarlo desde dentro). Este test cubre
    /// la mitad verificable sin eso: que la precondicion sobre la que se
    /// apoya el arreglo es cierta.
    ///
    /// Necesita una sesion de escritorio interactiva (bandeja de Explorer,
    /// RegisterHotKey), igual que las pruebas de `bandeja.rs` y
    /// `atajos.rs`. Se ejecuta a mano con `cargo test -- --ignored`.
    #[test]
    #[ignore = "necesita una sesion de escritorio interactiva. cargo test -- --ignored"]
    fn bandeja_y_atajos_se_sueltan_con_la_ventana_todavia_viva() {
        use crate::atajo::Atajo;
        use crate::atajos;
        use crate::bandeja::Bandeja;

        let ventana = VentanaMensajes::nueva().expect("deberia poder crearse");
        let hwnd = ventana.handle();

        let bandeja =
            Bandeja::nueva(hwnd, "PixPin Max — prueba de orden").expect("deberia añadirse");

        let raro: Atajo = "Ctrl+Alt+Shift+F21".parse().unwrap();
        let (registrados, fallidos) = atajos::registrar(hwnd, &[(atajos::ID_REGION, raro)]);
        assert!(fallidos.is_empty(), "no deberia chocar: {fallidos:?}");

        // Orden de declaracion: ventana, bandeja, registrados. Soltamos en
        // el orden inverso que Rust usaria al final del scope (y que main()
        // usa de verdad), pero paso a paso para poder comprobar el estado
        // de la ventana entre medias.

        // SAFETY: `hwnd` sigue vivo (nada lo ha destruido todavia);
        // consultar con IsWindow un handle vivo es siempre valido.
        let viva_al_empezar = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_al_empezar,
            "precondicion: la ventana debe existir antes de soltar nada"
        );

        drop(registrados);
        // SAFETY: `hwnd` sigue siendo la ventana de `ventana`, que todavia
        // no se ha soltado.
        let viva_tras_atajos = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_tras_atajos,
            "hallazgo 2: tras soltar los atajos (UnregisterHotKey), la \
             ventana debe seguir existiendo"
        );

        drop(bandeja);
        // SAFETY: igual que arriba: `ventana` sigue viva.
        let viva_tras_bandeja = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            viva_tras_bandeja,
            "hallazgo 2: tras soltar la bandeja (Shell_NotifyIconW NIM_DELETE), \
             la ventana debe seguir existiendo"
        );

        drop(ventana);
        // SAFETY: `hwnd` es el valor numerico de un handle ya destruido;
        // consultarlo con IsWindow es la operacion valida que comprueba
        // precisamente eso.
        let reconocida_tras_ventana = unsafe { IsWindow(Some(hwnd)) }.as_bool();
        assert!(
            !reconocida_tras_ventana,
            "tras soltar la ventana, ya no debe existir"
        );
    }
}
