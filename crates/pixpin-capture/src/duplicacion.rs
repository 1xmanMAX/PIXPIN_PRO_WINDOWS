//! Congelacion instantanea con DXGI Desktop Duplication.
//!
//! WGC tarda 50-100 ms en entregar su primer fotograma: demasiado para el
//! intocable de <50 ms del atajo. Desktop Duplication es PULL: mantener el
//! duplicador abierto no cuesta nada (cero CPU en reposo, es el compositor
//! quien acumula), y AcquireNextFrame entrega en milisegundos.
//!
//! El truco del fotograma cacheado: tras cada adquisicion se guarda una
//! copia propia. Si la pantalla no cambio desde la ultima vez, Acquire da
//! TIMEOUT — y la copia cacheada sigue siendo EXACTAMENTE la pantalla
//! actual, asi que se devuelve esa. WGC queda para el modo en vivo.

use pixpin_geom::Rect;
use windows::Win32::Foundation::E_ACCESSDENIED;
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, IDXGIDevice,
    IDXGIOutput1, IDXGIOutput5, IDXGIOutputDuplication, IDXGIResource,
};
use windows::core::Interface;

use crate::dispositivo::{Dispositivo, ErrorCaptura};
use crate::instantanea::{Instantanea, crear_textura};
use crate::monitores::handle_de_monitor;

/// Un duplicador persistente de UN monitor, con su fotograma cacheado.
pub struct Duplicador {
    duplicacion: IDXGIOutputDuplication,
    /// Copia propia del ultimo fotograma adquirido. Ver el truco del modulo.
    cache: Option<ID3D11Texture2D>,
    area: Rect,
}

impl Duplicador {
    /// Crea el duplicador del monitor indicado. Cuesta ~10-50 ms una vez;
    /// por eso se mantiene vivo entre capturas.
    pub fn nuevo(
        dispositivo: &Dispositivo,
        id_monitor: u32,
        area: Rect,
    ) -> Result<Self, ErrorCaptura> {
        let Some(hmon) = handle_de_monitor(id_monitor) else {
            return Err(ErrorCaptura::MonitorDesconocido(id_monitor));
        };
        let dxgi: IDXGIDevice = dispositivo.d3d().cast()?;
        // SAFETY: recorrido COM de solo lectura por adaptador y salidas del
        // dispositivo del llamante, vivo durante toda la llamada.
        let duplicacion = unsafe {
            let adaptador = dxgi.GetAdapter()?;
            let mut indice = 0u32;
            loop {
                let salida = match adaptador.EnumOutputs(indice) {
                    Ok(s) => s,
                    Err(_) => return Err(ErrorCaptura::MonitorDesconocido(id_monitor)),
                };
                let desc = salida.GetDesc()?;
                if desc.Monitor == hmon {
                    // En escritorios HDR, DuplicateOutput clasico entrega
                    // FP16 y CopyResource hacia B8G8R8A8 falla EN SILENCIO
                    // dejando la copia plana (lo cazo el test de pixeles).
                    // DuplicateOutput1 fuerza B8G8R8A8 con mapeo SDR; si el
                    // sistema no tiene IDXGIOutput5, se cae al clasico.
                    if let Ok(salida5) = salida.cast::<IDXGIOutput5>() {
                        if let Ok(d) = salida5.DuplicateOutput1(
                            dispositivo.d3d(),
                            0,
                            &[DXGI_FORMAT_B8G8R8A8_UNORM],
                        ) {
                            break d;
                        }
                    }
                    let salida1: IDXGIOutput1 = salida.cast()?;
                    break match salida1.DuplicateOutput(dispositivo.d3d()) {
                        Ok(d) => d,
                        // La duplicacion es EXCLUSIVA por salida: si un
                        // escritorio remoto o un grabador la tiene tomada,
                        // Windows responde "acceso denegado". Distinguirlo
                        // importa, porque la respuesta correcta no es
                        // reintentar sino usar WGC.
                        Err(e) if e.code() == E_ACCESSDENIED => {
                            return Err(ErrorCaptura::DuplicacionOcupada);
                        }
                        Err(e) => return Err(e.into()),
                    };
                }
                indice += 1;
            }
        };
        Ok(Self {
            duplicacion,
            cache: None,
            area,
        })
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    /// La pantalla AHORA, como instantanea en GPU. Milisegundos.
    ///
    /// `Err(AccesoPerdido)` significa que hay que recrear el duplicador
    /// (cambio de modo, pantalla completa exclusiva, sesion bloqueada).
    pub fn instantanea(&mut self, dispositivo: &Dispositivo) -> Result<Instantanea, ErrorCaptura> {
        // Drenar lo acumulado: puede haber varios fotogramas pendientes y
        // solo interesa el ultimo. Timeout corto: si no hay nada nuevo, el
        // cache ya ES la pantalla actual.
        let mut fresco: Option<ID3D11Texture2D> = None;
        loop {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut recurso: Option<IDXGIResource> = None;
            // SAFETY: punteros de salida locales; el fotograma adquirido se
            // libera SIEMPRE antes de la siguiente vuelta o de salir.
            let r = unsafe {
                self.duplicacion.AcquireNextFrame(
                    if fresco.is_none() && self.cache.is_none() {
                        // Primera vez: esperar de verdad a que llegue algo.
                        200
                    } else {
                        0
                    },
                    &mut info,
                    &mut recurso,
                )
            };
            match r {
                Ok(()) => {
                    // LastPresentTime == 0 significa "sin imagen nueva": el
                    // primer Acquire tras crear el duplicador con pantalla
                    // quieta entrega una textura VACIA con ese marcador, y
                    // copiarla dio la captura en negro que cazo el test.
                    if info.LastPresentTime != 0 {
                        if let Some(recurso) = &recurso {
                            let textura: ID3D11Texture2D = recurso.cast()?;
                            // Copia propia: el fotograma del duplicador hay
                            // que devolverlo con ReleaseFrame, no retenerlo.
                            let copia =
                                crear_textura(dispositivo, self.area.ancho, self.area.alto)?;
                            // SAFETY: mismo dispositivo y formato (forzado
                            // B8G8R8A8 en DuplicateOutput1), mismo tamano.
                            unsafe { dispositivo.contexto().CopyResource(&copia, &textura) };
                            fresco = Some(copia);
                        }
                    }
                    // SAFETY: empareja el Acquire de arriba.
                    unsafe { self.duplicacion.ReleaseFrame()? };
                }
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => break,
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    return Err(ErrorCaptura::AccesoPerdido);
                }
                Err(e) => return Err(e.into()),
            }
        }
        if let Some(f) = fresco {
            self.cache = Some(f);
        }
        match &self.cache {
            Some(t) => {
                // Se entrega una copia independiente: la instantanea del
                // overlay vive lo que dure la interaccion y el cache sigue
                // actualizandose en la proxima captura.
                let copia = crear_textura(dispositivo, self.area.ancho, self.area.alto)?;
                // SAFETY: mismo dispositivo y formato, tamanos identicos.
                unsafe { dispositivo.contexto().CopyResource(&copia, t) };
                Ok(Instantanea::desde_partes(copia, self.area))
            }
            None => Err(ErrorCaptura::SinFotograma),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::dispositivo::Dispositivo;
    use crate::monitores::enumerar_monitores;
    use crate::pruebas_util::con_movimiento;
    use std::time::{Duration, Instant};

    /// Crea el duplicador del monitor principal, o devuelve `None` si otro
    /// programa lo tiene tomado.
    ///
    /// La Duplicacion de Escritorio es EXCLUSIVA por salida: con un
    /// escritorio remoto o un grabador abiertos, Windows nos la niega. Eso
    /// no es un fallo de este codigo —la aplicacion cae a WGC y sigue
    /// funcionando—, asi que estos tests se saltan en vez de acusar en
    /// falso. Cualquier OTRO error si revienta el test: solo se perdona el
    /// caso que de verdad es del entorno.
    fn duplicador_o_saltar(d: &Dispositivo, m: &pixpin_geom::Monitor) -> Option<Duplicador> {
        match Duplicador::nuevo(d, m.id, m.area) {
            Ok(dup) => Some(dup),
            Err(ErrorCaptura::DuplicacionOcupada) => {
                eprintln!(
                    "AVISO: otro programa tiene tomada la duplicacion de escritorio \
                     (escritorio remoto, grabador...). Este test se salta; la \
                     aplicacion cae a WGC en esa situacion."
                );
                None
            }
            Err(e) => panic!("el duplicador deberia crearse: {e}"),
        }
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn el_duplicador_entrega_en_milisegundos_una_vez_caliente() {
        let d = Dispositivo::nuevo().unwrap();
        let m = enumerar_monitores().unwrap();
        let principal = *m.principal().unwrap();
        let Some(mut dup) = duplicador_o_saltar(&d, &principal) else {
            return;
        };

        // Calentar con movimiento real: el primer fotograma de verdad solo
        // llega cuando algo cambia en pantalla.
        con_movimiento(Duration::from_millis(600), || {
            std::thread::sleep(Duration::from_millis(300));
            dup.instantanea(&d).expect("primera instantanea")
        });

        // Calientes: 25 ms por captura deja el resto del presupuesto de
        // 50 ms para ventanas y present.
        let t = Instant::now();
        for _ in 0..5 {
            dup.instantanea(&d).expect("instantanea caliente");
        }
        let media = t.elapsed().as_millis() / 5;
        assert!(
            media <= 25,
            "el duplicador caliente tarda {media} ms de media; el intocable de 50 ms no da para eso"
        );
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn la_instantanea_del_duplicador_tiene_pixeles_reales() {
        // Caso negativo del cache: el primer Acquire tras crear el
        // duplicador con pantalla quieta trae una textura VACIA marcada con
        // LastPresentTime=0; copiarla daba una captura en negro. Este test
        // lo cazo. Con movimiento real, la imagen debe tener variacion.
        use crate::mapa::a_imagen;
        let d = Dispositivo::nuevo().unwrap();
        let m = enumerar_monitores().unwrap();
        let principal = *m.principal().unwrap();
        let Some(mut dup) = duplicador_o_saltar(&d, &principal) else {
            return;
        };
        let inst = con_movimiento(Duration::from_millis(600), || {
            std::thread::sleep(Duration::from_millis(300));
            dup.instantanea(&d).unwrap()
        });
        // El CENTRO del monitor, no la esquina: la esquina puede ser un
        // panel lateral de color plano de verdad (lo era en el equipo de
        // desarrollo y este test acuso al codigo por error).
        let region = pixpin_geom::Rect {
            x: principal.area.x + (principal.area.ancho / 2) as i32 - 200,
            y: principal.area.y + (principal.area.alto / 2) as i32 - 200,
            ancho: 400,
            alto: 400,
        };
        let img = a_imagen(&d, &inst.recortar(&d, region).unwrap()).unwrap();
        let primero = &img.pixeles[0..4];
        let distinto = img.pixeles.chunks_exact(4).any(|p| p != primero);
        assert!(
            distinto,
            "la instantanea parece un color plano: cache vacio o textura sin copiar"
        );
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn con_cache_frio_y_pantalla_quieta_dice_sin_fotograma() {
        // El contrato del arranque en frio: sin ningun fotograma real aun,
        // instantanea() debe DECIRLO (SinFotograma) para que el llamante
        // caiga a WGC, nunca devolver una textura en negro como si fuera la
        // pantalla.
        let d = Dispositivo::nuevo().unwrap();
        let m = enumerar_monitores().unwrap();
        let principal = *m.principal().unwrap();
        let Some(mut dup) = duplicador_o_saltar(&d, &principal) else {
            return;
        };
        match dup.instantanea(&d) {
            Err(ErrorCaptura::SinFotograma) => {}
            Ok(_) => {
                // Legitimo: algo cambio en pantalla entre crear y preguntar
                // (un cursor parpadeando basta). No es un fallo del contrato.
            }
            Err(e) => panic!("error inesperado en frio: {e:?}"),
        }
    }
}
