//! Gestos de raton con Alt en cualquier punto de la pantalla (D81).
//!
//! `Alt + boton izquierdo` y arrastrar selecciona y copia; `Alt + boton
//! derecho` y arrastrar selecciona y pinea. Los pidio el usuario en lugar de
//! los atajos de teclado `Ctrl+Alt+X` y `Ctrl+Alt+D`.
//!
//! Un gancho de raton de bajo nivel (`WH_MOUSE_LL`) ve la pulsacion antes
//! que la ventana de debajo. Si Alt esta pulsado (y ningun otro modificador),
//! se traga la pulsacion y se ESPERA: el gesto es arrastrar, no pulsar. Solo
//! cuando el cursor se aleja `UMBRAL_ARRASTRE` pixeles con el boton abajo se
//! avisa a la ventana de mensajes con `WM_GESTO`, y el bucle principal abre
//! el overlay ya con el arrastre en marcha desde el punto de la pulsacion.
//!
//! El usuario lo pidio asi tras probarlo: con el gesto disparado por la
//! pulsacion, elegir una ventana con el raton en el Alt+Tab (Alt sigue
//! abajo) abria un recorte. Si el boton se suelta sin haber arrastrado era
//! un clic corriente, y se le DEVUELVE a la ventana de debajo sintetizandolo:
//! la pulsacion ya se la habia tragado el gancho, y dejar pasar solo la
//! soltada entregaria medio clic.
//!
//! La soltada de un gesto ya anunciado NO se traga: para entonces el overlay
//! esta delante con la captura del raton y es quien tiene que recibirla.
//!
//! Mientras un overlay esta abierto el gancho se suspende: un Alt+clic
//! dentro del overlay es del overlay, no un gesto nuevo.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicUsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_MOUSE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HC_ACTION, HHOOK, MSLLHOOKSTRUCT, PostMessageW, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
};

use crate::ventana::WM_GESTO;

/// La ventana que recibe `WM_GESTO`, como entero para poder vivir en un
/// estatico (el gancho es una funcion libre sin estado propio).
static DESTINO: AtomicIsize = AtomicIsize::new(0);
/// Con el gancho suspendido las pulsaciones pasan sin tocarse.
static SUSPENDIDO: AtomicBool = AtomicBool::new(false);
/// Hay un boton tragado por el gancho y todavia sin soltar. Se lleva aqui
/// y no con `GetAsyncKeyState`: una pulsacion que el gancho se traga nunca
/// llega al estado del teclado del sistema, asi que para Windows el boton
/// jamas estuvo abajo.
static EN_CURSO: AtomicBool = AtomicBool::new(false);
/// Cuanto hay que alejarse de la pulsacion, en pixeles fisicos, para que sea
/// un arrastre. Ocho: por debajo, el temblor de la mano al hacer clic ya lo
/// cruza (Windows usa 4 para arrastrar iconos, pero ahi equivocarse no abre
/// nada encima de la pantalla); por encima, el recorte empezaria a notarse
/// tarde en una seleccion pequena.
pub const UMBRAL_ARRASTRE: i32 = 8;
/// Sin boton a la espera.
const NINGUNO: usize = usize::MAX;
/// El boton tragado que todavia no se sabe si es clic o arrastre (0, 1 o 2),
/// o `NINGUNO`.
static PENDIENTE: AtomicUsize = AtomicUsize::new(NINGUNO);
/// Donde se pulso el boton pendiente: el gesto arranca AHI, no donde se
/// cruzo el umbral, o a la seleccion le faltarian los primeros pixeles.
static ORIGEN_X: AtomicI32 = AtomicI32::new(0);
static ORIGEN_Y: AtomicI32 = AtomicI32::new(0);
/// El gesto pendiente ya se aviso con `WM_GESTO`.
static ANUNCIADO: AtomicBool = AtomicBool::new(false);
/// Marca de los clics que sintetiza este modulo, para dejarlos pasar en vez
/// de volver a tragarselos (Alt sigue pulsado cuando se devuelven).
const MARCA_PROPIA: usize = 0x5049_5850; // "PIXP"
/// Cuantos editores de lienzo hay abiertos. Dentro del editor, Alt + arrastrar
/// duplica lo elegido (D93); si el gancho se lo tragara, abriria una captura
/// encima del dibujo. Es un contador porque el editor puede estar abierto
/// desde un pin y desde el chat, en hilos distintos.
static EDITORES: AtomicUsize = AtomicUsize::new(0);

/// Mientras viva, los gestos con Alt no se interceptan. La toma el editor.
pub struct PausaGestos(());

impl PausaGestos {
    pub fn tomar() -> Self {
        EDITORES.fetch_add(1, Ordering::SeqCst);
        PausaGestos(())
    }
}

impl Drop for PausaGestos {
    fn drop(&mut self) {
        EDITORES.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Si el boton del ultimo gesto sigue pulsado. El overlay lo consulta al
/// abrirse para arrancar ya arrastrando.
pub fn gesto_en_curso() -> bool {
    EN_CURSO.load(Ordering::SeqCst)
}

/// El gancho instalado. Al soltarlo se desinstala y su hilo termina.
///
/// # Por que el gancho tiene un hilo para el solo
///
/// Un gancho de bajo nivel se ejecuta en el hilo que lo instalo, cuando ese
/// hilo bombea mensajes, y Windows le pone un tope: si el hilo tarda en
/// atender un evento, se lo salta, y a la de varias **retira el gancho sin
/// avisar**. Vivia en el hilo principal, y al llegar la pila de capturas el
/// trabajo que ese hilo hace al cerrar un recorte (bajar la imagen de la GPU,
/// el portapapeles, la miniatura, crear la ventana del icono) paso del tope
/// con el raton todavia moviendose: la primera captura salia y despues los
/// gestos morian sin dejar ni una linea en el registro, con el programa
/// respondiendo con normalidad. Medido el 2026-09-20 con gestos sinteticos.
///
/// En un hilo que solo bombea sus propios mensajes nada puede retrasarlo. El
/// procedimiento solo toca atomicos y publica `WM_GESTO` con `PostMessageW`,
/// que se puede llamar desde cualquier hilo, asi que no hizo falta cambiar
/// nada mas.
pub struct GanchoRaton {
    hilo: Option<std::thread::JoinHandle<()>>,
    id_hilo: u32,
}

impl GanchoRaton {
    /// Instala el gancho; los gestos llegan a `destino` como `WM_GESTO`.
    pub fn instalar(destino: HWND) -> windows::core::Result<Self> {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG};

        DESTINO.store(destino.0 as isize, Ordering::SeqCst);
        SUSPENDIDO.store(false, Ordering::SeqCst);
        let (tx, rx) = std::sync::mpsc::channel::<windows::core::Result<u32>>();
        let hilo = std::thread::Builder::new()
            .name("gancho-raton".into())
            .spawn(move || {
                // SAFETY: gancho global de bajo nivel con un procedimiento de
                // este modulo; sin hmodule ni hilo porque corre en ESTE hilo,
                // que es el que bombea justo debajo.
                let gancho: HHOOK =
                    match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(procedimiento), None, 0) } {
                        Ok(g) => g,
                        Err(e) => {
                            let _ = tx.send(Err(e));
                            return;
                        }
                    };
                // SAFETY: consulta del identificador del hilo actual.
                let _ = tx.send(Ok(unsafe { GetCurrentThreadId() }));
                let mut msg = MSG::default();
                // El bucle no reparte nada: este hilo no tiene ventanas. Solo
                // existe para que Windows tenga donde ejecutar el gancho, y
                // sale con el WM_QUIT que manda `Drop`.
                // SAFETY: GetMessageW escribe en un MSG propio.
                while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {}
                // SAFETY: desinstala el gancho que instalo este mismo hilo.
                unsafe {
                    let _ = UnhookWindowsHookEx(gancho);
                }
            })
            .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
        let id_hilo = rx.recv().map_err(|e| {
            windows::core::Error::new(windows::core::HRESULT(-1), e.to_string())
        })??;
        Ok(Self {
            hilo: Some(hilo),
            id_hilo,
        })
    }

    /// Deja pasar (o vuelve a interceptar) las pulsaciones con Alt.
    pub fn suspender(&self, suspendido: bool) {
        SUSPENDIDO.store(suspendido, Ordering::SeqCst);
    }
}

impl Drop for GanchoRaton {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        DESTINO.store(0, Ordering::SeqCst);
        // SAFETY: publicar WM_QUIT en la cola del hilo del gancho; si el hilo
        // ya no existe, falla y no hay nada que esperar.
        let avisado = unsafe { PostThreadMessageW(self.id_hilo, WM_QUIT, WPARAM(0), LPARAM(0)) };
        // El gancho lo desinstala su propio hilo al salir del bucle. Solo se
        // espera si el aviso llego: esperar a un hilo que no va a enterarse
        // colgaria el cierre del programa.
        if let (Ok(()), Some(hilo)) = (avisado, self.hilo.take()) {
            let _ = hilo.join();
        }
    }
}

fn pulsada(tecla: VIRTUAL_KEY) -> bool {
    // SAFETY: consulta del estado del teclado, sin precondiciones.
    unsafe { (GetAsyncKeyState(tecla.0 as i32) as u16 & 0x8000) != 0 }
}

/// Si la combinacion de modificadores es exactamente Alt.
fn solo_alt() -> bool {
    pulsada(VK_MENU)
        && !pulsada(VK_CONTROL)
        && !pulsada(VK_SHIFT)
        && !pulsada(VK_LWIN)
        && !pulsada(VK_RWIN)
}

/// Empaqueta un punto en un `LPARAM` como hacen `MAKELPARAM` y los mensajes
/// de raton: x en la palabra baja, y en la alta, ambas con signo.
pub fn empaquetar_punto(x: i32, y: i32) -> LPARAM {
    LPARAM(((x & 0xFFFF) | ((y & 0xFFFF) << 16)) as isize)
}

/// Si de `origen` a `p` hay ya un arrastre y no el temblor de un clic.
///
/// Por ejes y no por distancia euclidea: es lo que hace Windows con
/// `SM_CXDRAG`, y asi no hay raices ni desbordamientos que pensar.
pub fn es_arrastre(origen: (i32, i32), p: (i32, i32)) -> bool {
    (p.0 - origen.0).abs() >= UMBRAL_ARRASTRE || (p.1 - origen.1).abs() >= UMBRAL_ARRASTRE
}

/// Devuelve a la ventana de debajo el clic que el gancho se habia tragado.
fn devolver_clic(boton: usize) {
    let (abajo, arriba): (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) = match boton {
        0 => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        1 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        _ => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    };
    let evento = |flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: MARCA_PROPIA,
            },
        },
    };
    // SAFETY: SendInput recibe un array propio con el tamano correcto de la
    // estructura. Sin coordenadas: el clic cae donde esta el cursor, que no
    // se ha movido del sitio (por eso no fue un arrastre).
    unsafe {
        let _ = SendInput(&[evento(abajo), evento(arriba)], size_of::<INPUT>() as i32);
    }
}

extern "system" fn procedimiento(codigo: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let mensaje = wparam.0 as u32;
    if codigo != HC_ACTION as i32 {
        // SAFETY: pasar el evento al siguiente gancho de la cadena.
        return unsafe { CallNextHookEx(None, codigo, wparam, lparam) };
    }
    // SAFETY: con HC_ACTION, lParam apunta a un MSLLHOOKSTRUCT valido durante
    // la llamada.
    let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
    // El clic que este mismo modulo devuelve pasa sin mirarlo: Alt sigue
    // abajo, y tragarselo otra vez seria un bucle.
    if info.dwExtraInfo == MARCA_PROPIA {
        // SAFETY: pasar el evento al siguiente gancho de la cadena.
        return unsafe { CallNextHookEx(None, codigo, wparam, lparam) };
    }

    let soltado = match mensaje {
        WM_LBUTTONUP => Some(0usize),
        WM_RBUTTONUP => Some(1usize),
        WM_MBUTTONUP => Some(2usize),
        _ => None,
    };
    // Las soltadas se atienden este suspendido el gancho o no: cierran el
    // gesto, y el overlay necesita recibir la suya.
    if let Some(boton) = soltado {
        EN_CURSO.store(false, Ordering::SeqCst);
        if PENDIENTE.load(Ordering::SeqCst) == boton {
            PENDIENTE.store(NINGUNO, Ordering::SeqCst);
            if !ANUNCIADO.load(Ordering::SeqCst) {
                tracing::info!(
                    boton,
                    dx = info.pt.x - ORIGEN_X.load(Ordering::SeqCst),
                    dy = info.pt.y - ORIGEN_Y.load(Ordering::SeqCst),
                    "gesto: soltado sin arrastrar, se devuelve el clic"
                );
                // Se solto sin arrastrar: era un clic de la ventana de debajo.
                // La soltada se traga tambien, porque el clic entero se
                // devuelve sintetizado; dejarla pasar daria dos soltadas.
                devolver_clic(boton);
                return LRESULT(1);
            }
        }
    }

    // El arrastre que convierte la pulsacion pendiente en gesto.
    if mensaje == WM_MOUSEMOVE {
        let boton = PENDIENTE.load(Ordering::SeqCst);
        let destino = DESTINO.load(Ordering::SeqCst);
        let origen = (
            ORIGEN_X.load(Ordering::SeqCst),
            ORIGEN_Y.load(Ordering::SeqCst),
        );
        if boton != NINGUNO
            && destino != 0
            && !ANUNCIADO.load(Ordering::SeqCst)
            && es_arrastre(origen, (info.pt.x, info.pt.y))
        {
            ANUNCIADO.store(true, Ordering::SeqCst);
            tracing::info!(boton, "gesto: arrastre, se anuncia");
            // SAFETY: publicar un mensaje propio en una ventana propia.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(destino as *mut _)),
                    WM_GESTO,
                    WPARAM(boton),
                    empaquetar_punto(origen.0, origen.1),
                );
            }
        }
    }

    let pulsado = match mensaje {
        WM_LBUTTONDOWN => Some(0usize),
        WM_RBUTTONDOWN => Some(1usize),
        WM_MBUTTONDOWN => Some(2usize),
        _ => None,
    };
    if let Some(boton) = pulsado
        && pulsada(VK_MENU)
    {
        // Cuanto tardo el evento en llegar aqui. El gancho corre en el hilo
        // principal: si ese hilo esta ocupado, Windows entrega tarde (y pasado
        // su tope, NO entrega), y un gesto perdido asi no deja mas rastro.
        // SAFETY: consulta del reloj del sistema, sin precondiciones.
        let ahora = unsafe { windows::Win32::System::SystemInformation::GetTickCount() };
        let retraso_ms = ahora.wrapping_sub(info.time);
        let suspendido = SUSPENDIDO.load(Ordering::SeqCst);
        let editores = EDITORES.load(Ordering::SeqCst);
        let destino = DESTINO.load(Ordering::SeqCst);
        let solo = solo_alt();
        if !suspendido && editores == 0 && destino != 0 && solo {
            // Un pendiente viejo NO bloquea: si su soltada se perdio (el
            // gancho no la vio por el tope de Windows), quedarse esperandola
            // dejaria los gestos muertos hasta el proximo clic normal.
            let viejo = PENDIENTE.swap(boton, Ordering::SeqCst);
            ORIGEN_X.store(info.pt.x, Ordering::SeqCst);
            ORIGEN_Y.store(info.pt.y, Ordering::SeqCst);
            ANUNCIADO.store(false, Ordering::SeqCst);
            EN_CURSO.store(true, Ordering::SeqCst);
            tracing::info!(
                boton,
                x = info.pt.x,
                y = info.pt.y,
                retraso_ms,
                pendiente_viejo = viejo != NINGUNO,
                "gesto: pulsacion retenida"
            );
            // Tragar la pulsacion: si acaba en arrastre, la ventana de debajo
            // no debe haber reaccionado; si acaba en clic, se le devuelve.
            return LRESULT(1);
        }
        // Con Alt abajo y sin gesto: que quede dicho por que. Es el unico
        // rastro que deja un gesto que «no funciona».
        tracing::info!(
            boton,
            suspendido,
            editores,
            sin_destino = destino == 0,
            ctrl = pulsada(VK_CONTROL),
            shift = pulsada(VK_SHIFT),
            win = pulsada(VK_LWIN) || pulsada(VK_RWIN),
            retraso_ms,
            "gesto: pulsacion con Alt NO retenida"
        );
    }
    // SAFETY: pasar el evento al siguiente gancho de la cadena.
    unsafe { CallNextHookEx(None, codigo, wparam, lparam) }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_pausa_del_editor_se_suelta_al_cerrarlo() {
        let antes = EDITORES.load(Ordering::SeqCst);
        {
            let _a = PausaGestos::tomar();
            let _b = PausaGestos::tomar();
            assert_eq!(EDITORES.load(Ordering::SeqCst), antes + 2, "dos editores");
        }
        assert_eq!(
            EDITORES.load(Ordering::SeqCst),
            antes,
            "cerrados los dos, los gestos vuelven"
        );
    }

    #[test]
    fn un_clic_con_alt_no_es_un_gesto_y_un_arrastre_si() {
        // Lo que fallaba: elegir una ventana con el raton en el Alt+Tab
        // abria un recorte. El temblor de un clic no cruza el umbral.
        assert!(!es_arrastre((500, 500), (500, 500)));
        assert!(!es_arrastre((500, 500), (503, 497)));
        assert!(!es_arrastre((500, 500), (507, 507)), "justo por debajo");
        // Arrastrar en cualquier direccion si, tambien en un solo eje y
        // hacia arriba o hacia la izquierda.
        assert!(es_arrastre((500, 500), (508, 500)));
        assert!(es_arrastre((500, 500), (500, 492)));
        assert!(es_arrastre((-1900, 40), (-1950, 40)), "monitor de la izquierda");
    }

    #[test]
    fn el_punto_empaquetado_se_desempaqueta_con_signo() {
        // Un monitor a la izquierda del principal tiene x negativa; la
        // ventana de mensajes lo desempaqueta igual que aqui.
        let l = empaquetar_punto(-120, 45);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (-120, 45));

        let l = empaquetar_punto(2999, 1999);
        let x = (l.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        assert_eq!((x, y), (2999, 1999));
    }
}
