//! El hilo de UI Automation: candidatos de snap con cache, sin bloquear jamas.
//!
//! UIA es COM entre procesos: una consulta puede tardar decenas de
//! milisegundos o colgarse si la aplicacion destino no responde. La regla de
//! la spec: el overlay NUNCA espera. pedir() deja la posicion en un canal de
//! capacidad 1 (la nueva pisa a la vieja); el hilo contesta cuando puede,
//! deja el resultado bajo un Mutex breve y despierta al bucle modal con
//! PostMessage(MSG_DESPIERTA).

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use pixpin_geom::{Candidato, Punto, Rect};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetClassNameW, PostMessageW, WindowFromPoint,
};

use crate::overlay::MSG_DESPIERTA;

enum Peticion {
    Cursor(Punto),
    Parar,
}

pub struct Uia {
    envia: SyncSender<Peticion>,
    resultado: Arc<Mutex<Vec<Candidato>>>,
    hilo: Option<JoinHandle<()>>,
}

impl Uia {
    pub fn nueva(notificar: HWND) -> Uia {
        // Capacidad 1: si el hilo esta ocupado, la peticion vieja se pisa.
        let (envia, recibe) = sync_channel::<Peticion>(1);
        let resultado = Arc::new(Mutex::new(Vec::new()));
        let resultado_hilo = Arc::clone(&resultado);
        // HWND no es Send en el crate windows: se pasa el valor crudo. Es
        // seguro porque PostMessageW tolera ventanas ya destruidas.
        let notificar_crudo = notificar.0 as isize;
        let hilo = std::thread::Builder::new()
            .name("pixpin-uia".into())
            .spawn(move || trabajar(recibe, resultado_hilo, notificar_crudo))
            .expect("el hilo UIA deberia poder crearse");
        Uia {
            envia,
            resultado,
            hilo: Some(hilo),
        }
    }

    /// NUNCA bloquea. Si el canal esta lleno, la peticion nueva se descarta:
    /// la siguiente llegara con el proximo movimiento del raton, y lo unico
    /// que importa es la ultima posicion.
    pub fn pedir(&self, cursor: Punto) {
        match self.envia.try_send(Peticion::Cursor(cursor)) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    /// Copia lo ultimo que el hilo haya resuelto. El Mutex se toma solo
    /// para clonar el Vec: microsegundos.
    pub fn candidatos(&self) -> Vec<Candidato> {
        self.resultado.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn detener(mut self) {
        let _ = self.envia.send(Peticion::Parar);
        if let Some(h) = self.hilo.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Uia {
    fn drop(&mut self) {
        // Si detener() no se llamo, al menos pedir la parada sin join: no
        // bloquear un drop es mas importante que la limpieza perfecta.
        let _ = self.envia.try_send(Peticion::Parar);
    }
}

fn trabajar(recibe: Receiver<Peticion>, resultado: Arc<Mutex<Vec<Candidato>>>, notificar: isize) {
    // SAFETY: cada hilo inicializa COM una vez y lo libera al salir.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let automatizacion: Option<IUIAutomation> =
        // SAFETY: CoCreateInstance del CLSID documentado; None si no hay UIA.
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok() };

    let mut cache_raiz: Option<(isize, Vec<Candidato>)> = None;

    while let Ok(Peticion::Cursor(p)) = recibe.recv() {
        let mut candidatos = Vec::new();
        if let Some((raiz, respaldo)) = candidato_respaldo(p) {
            candidatos.push(respaldo);
            // Arbol UIA, con cache por ventana raiz.
            if let Some(auto) = &automatizacion {
                match &cache_raiz {
                    Some((r, arbol)) if *r == raiz => candidatos.extend(arbol.iter().copied()),
                    _ => {
                        let arbol = arbol_uia(auto, p);
                        candidatos.extend(arbol.iter().copied());
                        cache_raiz = Some((raiz, arbol));
                    }
                }
            }
        }
        if let Ok(mut r) = resultado.lock() {
            *r = candidatos;
        }
        if notificar != 0 {
            // SAFETY: PostMessageW tolera ventanas destruidas; el mensaje es
            // el WM_APP privado del overlay.
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(notificar as *mut _)),
                    MSG_DESPIERTA,
                    Default::default(),
                    Default::default(),
                );
            }
        }
    }
    // El objeto COM debe soltarse ANTES de CoUninitialize: si se dropeara al
    // salir de la funcion, su Release() correria sobre un apartamento ya
    // destruido y el proceso muere con ACCESS_VIOLATION. Se descubrio con el
    // test de detener en frio, donde ningun otro hilo mantenia vivo el MTA.
    drop(automatizacion);
    // SAFETY: empareja el CoInitializeEx de arriba, y para entonces ya no
    // vive ninguna interfaz COM de este hilo (el drop de la linea anterior
    // es la garantia, no una esperanza).
    unsafe { CoUninitialize() };
}

/// La ventana raiz bajo el punto y su rectangulo DWM. Nunca GetWindowRect:
/// incluye la sombra invisible y el recuadro saldria mas grande por lado.
fn candidato_respaldo(p: Punto) -> Option<(isize, Candidato)> {
    // SAFETY: consultas de solo lectura sobre el estado global de ventanas.
    unsafe {
        let bajo = WindowFromPoint(POINT { x: p.x, y: p.y });
        if bajo.is_invalid() {
            return None;
        }
        let raiz = GetAncestor(bajo, GA_ROOT);
        // Excluir los propios overlays o el snap se ajustaria a si mismo.
        let mut clase = [0u16; 32];
        let n = GetClassNameW(raiz, &mut clase);
        if n > 0 && String::from_utf16_lossy(&clase[..n as usize]) == "PixPinOverlay" {
            return None;
        }
        let mut r = RECT::default();
        DwmGetWindowAttribute(
            raiz,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut RECT as *mut _,
            size_of::<RECT>() as u32,
        )
        .ok()?;
        Some((
            raiz.0 as isize,
            Candidato {
                rect: Rect {
                    x: r.left,
                    y: r.top,
                    ancho: (r.right - r.left).max(0) as u32,
                    alto: (r.bottom - r.top).max(0) as u32,
                },
                profundidad: 0,
            },
        ))
    }
}

/// El elemento bajo el punto y sus ancestros, como candidatos de
/// profundidad creciente. Cualquier fallo COM corta y devuelve lo reunido.
fn arbol_uia(auto: &IUIAutomation, p: Punto) -> Vec<Candidato> {
    let mut lista = Vec::new();
    // SAFETY: llamadas COM de solo lectura; cualquier error corta con ok().
    unsafe {
        let Ok(elemento) = auto.ElementFromPoint(POINT { x: p.x, y: p.y }) else {
            return lista;
        };
        let Ok(caminante) = auto.ControlViewWalker() else {
            return lista;
        };
        let caminante: IUIAutomationTreeWalker = caminante;
        // Del elemento hacia la raiz, apuntando el rectangulo de cada nivel;
        // la profundidad real se corrige al final, cuando se sabe el total.
        let mut cadena: Vec<Rect> = Vec::new();
        let mut actual: Option<IUIAutomationElement> = Some(elemento);
        while let Some(e) = actual {
            if let Ok(r) = e.CurrentBoundingRectangle() {
                cadena.push(Rect {
                    x: r.left,
                    y: r.top,
                    ancho: (r.right - r.left).max(0) as u32,
                    alto: (r.bottom - r.top).max(0) as u32,
                });
            }
            actual = caminante.GetParentElement(&e).ok();
            // Cota dura: un arbol UIA real no pasa de ~30 niveles; sin esto,
            // un GetParentElement ciclico colgaria el hilo para siempre.
            if cadena.len() > 32 {
                break;
            }
        }
        let total = cadena.len() as u16;
        for (i, rect) in cadena.into_iter().enumerate() {
            lista.push(Candidato {
                rect,
                profundidad: total - i as u16,
            });
        }
    }
    lista
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_geom::Punto;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::HWND;

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn pedir_no_bloquea_aunque_se_pida_en_rafaga() {
        // El contrato central: 200 peticiones seguidas deben costar
        // microsegundos cada una, este o no este el hilo UIA ocupado. Si
        // pedir() esperara la respuesta, esto tardaria segundos y el
        // overlay se congelaria igual que la competencia.
        let uia = Uia::nueva(HWND::default());
        let inicio = Instant::now();
        for i in 0..200 {
            uia.pedir(Punto { x: i, y: i });
        }
        assert!(
            inicio.elapsed() < Duration::from_millis(50),
            "pedir() esta bloqueando: {:?}",
            inicio.elapsed()
        );
        uia.detener();
    }

    #[test]
    #[ignore = "necesita sesion de escritorio; ejecutar con --ignored"]
    fn sobre_el_escritorio_siempre_hay_al_menos_el_respaldo() {
        // Puede no haber UIA (politicas, apps que no lo implementan), pero el
        // respaldo por ventana + DWM tiene que aparecer siempre.
        let uia = Uia::nueva(HWND::default());
        uia.pedir(Punto { x: 10, y: 10 });
        // Espera acotada: el hilo contesta o el test falla con lista vacia.
        let limite = Instant::now() + Duration::from_secs(3);
        let mut candidatos = Vec::new();
        while Instant::now() < limite {
            candidatos = uia.candidatos();
            if !candidatos.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        uia.detener();
        assert!(!candidatos.is_empty(), "ni siquiera llego el respaldo DWM");
        assert!(
            candidatos.iter().any(|c| c.profundidad == 0),
            "falta el candidato de profundidad 0 (la ventana raiz)"
        );
        assert!(
            candidatos.iter().all(|c| !c.rect.esta_vacio()),
            "ningun candidato puede tener area cero: {candidatos:?}"
        );
    }

    #[test]
    fn detener_no_se_cuelga_con_el_hilo_ocioso() {
        // No necesita escritorio: crea y detiene. Si detener() hiciera un
        // join sin cerrar el canal, este test se quedaria colgado.
        let uia = Uia::nueva(HWND::default());
        uia.detener();
    }
}
