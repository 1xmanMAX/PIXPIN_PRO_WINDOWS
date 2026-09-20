//! Una superficie de composicion por ventana de overlay.
//!
//! `WS_EX_NOREDIRECTIONBITMAP` quita a la ventana su superficie GDI: lo
//! unico que se ve es lo que este swapchain presenta a traves de
//! DirectComposition. Es el camino sin parpadeo y sin copias del escritorio
//! moderno, el mismo que usa QuickView — la tecnica, no el codigo.
//!
//! El present es A DEMANDA: se presenta cuando algo cambio, nunca en un
//! bucle de fotogramas. De ahi sale el 0% de CPU en reposo del overlay.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap1;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::DirectComposition::{
    DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR, DCompositionCreateDevice, IDCompositionDevice,
    IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
    DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2,
    IDXGISwapChain1,
};
use windows::core::Interface;

use crate::motor::{ErrorRender, MotorRender};

/// Lo que hace falta saber del reloj de DWM para pintar justo a tiempo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RitmoComposicion {
    /// Milisegundos que faltan para la siguiente composicion.
    pub hasta_el_plazo_ms: f32,
    /// Cada cuanto compone DWM (16,67 ms a 60 Hz).
    pub periodo_ms: f32,
}

/// La capa de la INTERFAZ: la barra, el panel y la ruta del universo, en un
/// visual propio ENCIMA del de la escena.
///
/// Existe por A3: si la escena se desplaza moviendo su visual, lo que no es
/// escena no puede ir en la misma superficie o la barra de herramientas se
/// iria con el lienzo. Es una superficie de composicion y no una segunda
/// swapchain a proposito: una swapchain tiene dos buffers (el doble de
/// memoria de video) y la interfaz se repinta cuando cambia, no sesenta
/// veces por segundo.
struct CapaInterfaz {
    _visual: IDCompositionVisual,
    superficie: windows::Win32::Graphics::DirectComposition::IDCompositionSurface,
    tamano: (u32, u32),
}

pub struct Superficie {
    dcomp: IDCompositionDevice,
    _objetivo: IDCompositionTarget,
    /// El visual de la ESCENA (lo que lleva la swapchain). Es el que se
    /// mueve al desplazar y el que se estira al acercar.
    visual: IDCompositionVisual,
    /// La raiz, para colgar de ella la capa de interfaz.
    _raiz: IDCompositionVisual,
    swapchain: IDXGISwapChain1,
    /// Tamano real de los buffers. Puede ser MAYOR que la ventana: la
    /// composicion recorta al area de la ventana, y asi un zoom no tiene
    /// que reasignar memoria de video en cada fotograma (histeresis).
    asignado: std::cell::Cell<(u32, u32)>,
    /// Hay una transformada de estirado puesta en el visual.
    estirada: std::cell::Cell<bool>,
    /// La escala y el desplazamiento del estirado (`estirar`), guardados
    /// aparte del paneo porque se combinan en una sola matriz.
    estiramiento: std::cell::Cell<(f32, f32, f32, f32)>,
    /// A3: cuantos pixeles de mas tiene la superficie de escena POR CADA
    /// LADO. Es el colchon que permite desplazar moviendo el visual sin
    /// ensenar borde vacio; a 0 la superficie se comporta como siempre.
    margen: u32,
    /// A3: cuanto se ha corrido el visual de la escena desde el ultimo
    /// repintado, en pixeles de ventana.
    desplazamiento: std::cell::Cell<(f32, f32)>,
    /// Banderas con las que nacio la swapchain: `ResizeBuffers` tiene que
    /// repetirlas o pierde la espera de fotograma.
    banderas: DXGI_SWAP_CHAIN_FLAG,
    /// La senal de «ya se puede presentar otro fotograma», solo en la
    /// superficie de baja latencia. Se cierra en `Drop`.
    senal: Option<windows::Win32::Foundation::HANDLE>,
    interfaz: std::cell::RefCell<Option<CapaInterfaz>>,
}

impl Drop for Superficie {
    fn drop(&mut self) {
        if let Some(s) = self.senal.take() {
            // SAFETY: el handle lo dio GetFrameLatencyWaitableObject y es
            // nuestro; se cierra una sola vez.
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(s);
            }
        }
    }
}

impl Superficie {
    /// La ventana del llamante debe sobrevivir a la Superficie (obligacion
    /// del llamante: en el overlay, la ventana posee a su Superficie).
    pub fn nueva(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, false, 0)
    }

    /// Superficie para dibujar a mano: latencia de fotograma 1 y senal de
    /// espera (`senal_fotograma`).
    ///
    /// Por defecto DXGI deja encolar hasta tres fotogramas. Un editor que
    /// pinta en 3 ms los llena, y cada trazo sale con esos fotogramas de
    /// retraso: la tinta va detras del raton. Con la cola en 1 y esperando a
    /// la senal ANTES de leer el raton, los puntos se leen lo mas tarde
    /// posible y se ven en el siguiente refresco (tecnica de «frame latency
    /// waitable object» de Microsoft para aplicaciones de tinta y juegos).
    pub fn nueva_baja_latencia(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, true, 0)
    }

    /// Como `nueva_baja_latencia`, pero con **colchon** alrededor de la
    /// escena y una capa aparte para la interfaz (A3).
    ///
    /// La superficie de escena se hace `margen` pixeles mas grande por cada
    /// lado y su visual nace corrido `-margen`: lo que se ve por la ventana
    /// sigue siendo el centro. A cambio, desplazar el lienzo hasta `margen`
    /// pixeles es mover el visual (`desplazar_escena`) en vez de repintar la
    /// escena entera, que es lo que costaba 15 ms con 2.000 elementos y
    /// 83 ms con 10.000 aunque no se creara ni un objeto de Direct2D.
    ///
    /// La barra y el panel van en `pintar_interfaz`, que no se mueve.
    pub fn nueva_con_capas(
        motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
        margen: u32,
    ) -> Result<Self, ErrorRender> {
        Self::crear(motor, d3d, hwnd, ancho, alto, true, margen)
    }

    /// El handle crudo que se puede esperar (junto con los mensajes) hasta
    /// que DXGI admita otro fotograma. `None` en la superficie normal.
    pub fn senal_fotograma(&self) -> Option<isize> {
        self.senal.map(|h| h.0 as isize)
    }

    /// Pixeles de colchon por lado. 0 en las superficies de siempre.
    pub fn margen(&self) -> f32 {
        self.margen as f32
    }

    /// Si esta superficie tiene capa de interfaz separada, es decir, si la
    /// escena se puede mover sin llevarse la barra.
    pub fn tiene_capas(&self) -> bool {
        self.margen > 0
    }

    fn crear(
        _motor: &MotorRender,
        d3d: &ID3D11Device,
        hwnd: HWND,
        ancho: u32,
        alto: u32,
        baja_latencia: bool,
        margen: u32,
    ) -> Result<Self, ErrorRender> {
        let (ancho, alto) = (ancho.max(1), alto.max(1));
        // La superficie de la escena lleva el colchon; la ventana sigue
        // siendo `ancho` x `alto` y la composicion recorta a ella.
        let (ancho_escena, alto_escena) = (ancho + margen * 2, alto + margen * 2);
        use windows::Win32::Graphics::Dxgi::{
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, IDXGISwapChain2,
        };
        let banderas = if baja_latencia {
            DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT
        } else {
            DXGI_SWAP_CHAIN_FLAG(0)
        };
        let dxgi: IDXGIDevice = d3d.cast().map_err(|_| ErrorRender::SinDxgi)?;
        // SAFETY: el adaptador y la factoria se obtienen del dispositivo del
        // llamante, vivo durante toda la llamada.
        let fabrica: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: ancho_escena,
            Height: alto_escena,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            // Dos buffers y flip: el minimo que permite componer sin copia.
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            Scaling: DXGI_SCALING_STRETCH,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: banderas.0 as u32,
            ..Default::default()
        };
        // SAFETY: swapchain DE COMPOSICION (sin HWND propio): es el unico
        // tipo valido para NOREDIRECTIONBITMAP; parametros documentados.
        let swapchain = unsafe { fabrica.CreateSwapChainForComposition(&dxgi, &desc, None)? };
        let senal = if baja_latencia {
            // SAFETY: la swapchain se creo con la bandera de espera, que es
            // la condicion de las dos llamadas; el handle es nuestro.
            unsafe {
                let s2: IDXGISwapChain2 = swapchain.cast()?;
                s2.SetMaximumFrameLatency(1)?;
                Some(s2.GetFrameLatencyWaitableObject())
            }
        } else {
            None
        };

        // SAFETY: dispositivo de composicion sobre el mismo DXGI; el target
        // toma la ventana del llamante (ver la obligacion en el doc de
        // `nueva`).
        let dcomp: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi)? };
        // SAFETY: los objetos se acaban de crear y estan vivos; Commit
        // publica el arbol de composicion.
        // Con capas, la raiz es un visual vacio del que cuelgan la escena y
        // la interfaz. Sin capas, la raiz ES el visual de la escena, igual
        // que siempre: una superficie de pin no paga ni un objeto de mas.
        let (objetivo, visual, raiz, interfaz) = unsafe {
            let objetivo = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swapchain)?;
            if margen == 0 {
                objetivo.SetRoot(&visual)?;
                dcomp.Commit()?;
                (objetivo, visual.clone(), visual, None)
            } else {
                let raiz = dcomp.CreateVisual()?;
                raiz.AddVisual(&visual, true, None)?;
                // El colchon: el visual nace corrido para que por la ventana
                // se vea el centro de la superficie y no su esquina.
                visual.SetOffsetX2(-(margen as f32))?;
                visual.SetOffsetY2(-(margen as f32))?;
                let sup = dcomp.CreateSurface(
                    ancho,
                    alto,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?;
                let visual_interfaz = dcomp.CreateVisual()?;
                visual_interfaz.SetContent(&sup)?;
                // `true` = encima de todo lo que ya cuelga de la raiz.
                raiz.AddVisual(&visual_interfaz, true, None)?;
                objetivo.SetRoot(&raiz)?;
                dcomp.Commit()?;
                (
                    objetivo,
                    visual,
                    raiz,
                    Some(CapaInterfaz {
                        _visual: visual_interfaz,
                        superficie: sup,
                        tamano: (ancho, alto),
                    }),
                )
            }
        };

        Ok(Self {
            dcomp,
            _objetivo: objetivo,
            visual,
            _raiz: raiz,
            swapchain,
            asignado: std::cell::Cell::new((ancho_escena, alto_escena)),
            estirada: std::cell::Cell::new(false),
            estiramiento: std::cell::Cell::new((1.0, 1.0, 0.0, 0.0)),
            margen,
            desplazamiento: std::cell::Cell::new((0.0, 0.0)),
            banderas,
            senal,
            interfaz: std::cell::RefCell::new(interfaz),
        })
    }

    /// Estira lo YA dibujado sin volver a dibujarlo: el compositor escala la
    /// textura en la GPU. Es como se hace un zoom fluido — redibujar el
    /// contenido en cada fotograma de la animacion es lo que producia
    /// tirones en equipos con graficos integrados.
    ///
    /// `escala_x`/`escala_y` multiplican, `dx`/`dy` desplazan despues, todo
    /// en pixeles de la ventana. Mientras hay transformada, el filtro pasa a
    /// ser el barato: es un fotograma intermedio de una animacion, y el
    /// nitido llega al terminar, con el repintado de verdad.
    pub fn estirar(&self, escala_x: f32, escala_y: f32, dx: f32, dy: f32) {
        self.estiramiento.set((escala_x, escala_y, dx, dy));
        // SAFETY: el visual es propio y sigue vivo; un error solo significa
        // que el fotograma sale sin el filtro barato.
        unsafe {
            let _ = self
                .visual
                .SetBitmapInterpolationMode(DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR);
        }
        self.estirada.set(true);
        self.aplicar_transformada();
    }

    /// Si hay una transformada de estirado puesta, es decir, si lo que se ve
    /// es una textura escalada y no un dibujo nitido.
    pub fn esta_estirada(&self) -> bool {
        self.estirada.get()
    }

    /// Deshace `estirar`. Solo toca la composicion si habia algo que
    /// deshacer: un Commit por fotograma en balde tambien cuesta.
    pub fn dejar_de_estirar(&self) {
        if !self.estirada.get() {
            return;
        }
        self.estiramiento.set((1.0, 1.0, 0.0, 0.0));
        self.estirada.set(false);
        self.aplicar_transformada();
    }

    /// A3: corre el visual de la ESCENA `dx`/`dy` pixeles de ventana, sin
    /// repintar nada y sin tocar la interfaz.
    ///
    /// Es todo el truco: un paneo deja de ser «pintar lo visible sesenta
    /// veces por segundo» y pasa a ser una matriz y un `Commit`. Lo que
    /// asoma por el borde sale del colchon de `margen`; pasado el colchon,
    /// el llamante tiene que repintar (`desplazamiento_valido`).
    pub fn desplazar_escena(&self, dx: f32, dy: f32) {
        if self.margen == 0 || self.desplazamiento.get() == (dx, dy) {
            return;
        }
        self.desplazamiento.set((dx, dy));
        self.aplicar_transformada();
    }

    /// Cuanto lleva corrido el visual de la escena.
    pub fn desplazamiento(&self) -> (f32, f32) {
        self.desplazamiento.get()
    }

    /// Si un desplazamiento de `dx`/`dy` sigue cubierto por el colchon. A
    /// partir de aqui asomaria el borde vacio y hay que repintar.
    pub fn desplazamiento_valido(&self, dx: f32, dy: f32) -> bool {
        self.margen > 0 && dx.abs() <= self.margen as f32 && dy.abs() <= self.margen as f32
    }

    /// Vuelve a dejar la escena en su sitio. Lo llama el repintado nitido:
    /// lo que se acaba de pintar ya esta en la camara nueva.
    pub fn reponer_escena(&self) {
        if self.desplazamiento.get() == (0.0, 0.0) {
            return;
        }
        self.desplazamiento.set((0.0, 0.0));
        self.aplicar_transformada();
    }

    /// La unica matriz del visual de la escena: colchon, paneo y estirado
    /// juntos.
    ///
    /// Van juntos y no en dos propiedades distintas porque el orden importa:
    /// primero se lleva la superficie a coordenadas de ventana (quitarle el
    /// colchon y sumarle el paneo) y solo despues se estira. Con
    /// `SetOffsetX2` por un lado y `SetTransform2` por otro, un zoom a mitad
    /// de un paneo escalaria tambien el desplazamiento.
    fn aplicar_transformada(&self) {
        let (sx, sy, ex, ey) = self.estiramiento.get();
        let (dx, dy) = self.desplazamiento.get();
        let m = self.margen as f32;
        let m = windows_numerics::Matrix3x2 {
            M11: sx,
            M12: 0.0,
            M21: 0.0,
            M22: sy,
            M31: (dx - m) * sx + ex,
            M32: (dy - m) * sy + ey,
        };
        // SAFETY: la matriz vive durante la llamada; el visual y el
        // dispositivo son propios y siguen vivos. Un error solo significa
        // que el fotograma sale sin mover.
        unsafe {
            let _ = self.visual.SetTransform2(&m);
            let _ = self.dcomp.Commit();
        }
    }

    /// Pinta la capa de la INTERFAZ (barra, panel, ruta) y la publica.
    ///
    /// El cierre recibe el `Pintor` y el desplazamiento que tiene que sumar
    /// a todo lo que dibuje: DirectComposition puede devolver la superficie
    /// dentro de una textura mas grande, y no sumarlo pintaria la barra en
    /// el sitio equivocado justo en los equipos donde el atlas no empieza en
    /// cero.
    ///
    /// Hace su propio `Commit`, asi que lo que se pinte aqui aparece con el
    /// siguiente fotograma de la escena o solo, si la escena no cambia.
    pub fn pintar_interfaz(
        &self,
        motor: &MotorRender,
        dibuja: impl FnOnce(&crate::lienzo::Pintor<'_>, (f32, f32)),
    ) -> Result<(), ErrorRender> {
        use windows::Win32::Foundation::POINT;
        let prestamo = self.interfaz.borrow();
        let Some(capa) = prestamo.as_ref() else {
            return Ok(());
        };
        let mut offset = POINT::default();
        // SAFETY: la superficie es propia y esta viva; `offset` es local.
        // `BeginDraw` sin rectangulo actualiza la superficie entera, que es
        // lo que hace falta porque la interfaz es casi toda transparente y
        // un trozo sin pintar dejaria basura del fotograma anterior.
        let textura: ID3D11Texture2D =
            unsafe { capa.superficie.BeginDraw(None, &mut offset as *mut POINT)? };
        let destino = motor.destino_backbuffer(&textura)?;
        let d = (offset.x as f32, offset.y as f32);
        let (w, h) = capa.tamano;
        let r = motor.dibujar(&destino, |p| {
            // El recorte va ANTES de limpiar y con la matriz en identidad:
            // `Clear` borra el destino ENTERO dentro del recorte, y el
            // destino puede ser un atlas compartido con otras superficies de
            // composicion. Sin el recorte, limpiar la interfaz podria borrar
            // lo que no es nuestro.
            p.empujar_recorte(crate::lienzo::RectF {
                x: d.0,
                y: d.1,
                ancho: w as f32,
                alto: h as f32,
            });
            p.limpiar_transparente();
            p.desplazar(d.0, d.1);
            dibuja(p, d);
            p.desplazar(0.0, 0.0);
            p.soltar_recorte();
        });
        // SAFETY: cierra el `BeginDraw` de arriba, pase lo que pase en el
        // dibujo: dejarlo abierto bloquearia la siguiente vuelta.
        unsafe {
            let _ = capa.superficie.EndDraw();
            let _ = self.dcomp.Commit();
        }
        r?;
        Ok(())
    }

    /// El tamano de la capa de interfaz, si la hay.
    pub fn tamano_interfaz(&self) -> Option<(u32, u32)> {
        self.interfaz.borrow().as_ref().map(|c| c.tamano)
    }

    /// Garantiza buffers de al menos `ancho` x `alto`. Si hay que crecer,
    /// crece con margen (un cuarto mas) para que el siguiente paso de un
    /// zoom no vuelva a reasignar. Encoger lo hace `compactar`, al acabar
    /// el gesto: durante el gesto, encoger cada fotograma es tan caro como
    /// crecer.
    pub fn asegurar(&self, ancho: u32, alto: u32) -> Result<(), ErrorRender> {
        let (aw, ah) = self.asignado.get();
        if ancho <= aw && alto <= ah {
            return Ok(());
        }
        let nw = if ancho > aw {
            (ancho + ancho / 4).min(16_384)
        } else {
            aw
        };
        let nh = if alto > ah {
            (alto + alto / 4).min(16_384)
        } else {
            ah
        };
        self.redimensionar(nw, nh)
    }

    /// Devuelve los buffers al tamano justo si estan muy sobrados (mas del
    /// doble en alguna dimension). Para el final de un gesto. Devuelve si
    /// los toco: entonces el contenido se perdio y hay que repintar.
    pub fn compactar(&self, ancho: u32, alto: u32) -> Result<bool, ErrorRender> {
        let (aw, ah) = self.asignado.get();
        if aw > ancho.max(1) * 2 || ah > alto.max(1) * 2 {
            self.redimensionar(ancho, alto)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lo que DirectComposition sabe del ritmo de DWM: cuanto falta para la
    /// siguiente composicion y cada cuanto compone.
    ///
    /// Es la pieza que le faltaba a `pixpin_tinta::ritmo::Planificador` para
    /// decidir a que hora empezar el fotograma (B1 de la investigacion del
    /// 2026-09-19): hoy se pinta nada mas dispararse la senal de la
    /// swapchain y el fotograma se queda esperando quieto los 10-13 ms que
    /// faltan para que DWM lo componga, con los puntos del lapiz
    /// envejeciendo dentro.
    ///
    /// Devuelve `None` si DirectComposition no lo sabe (lo dice devolviendo
    /// una frecuencia de 0, por ejemplo con la ventana tapada): el llamante
    /// se queda con el comportamiento de siempre, que es pintar ya.
    pub fn ritmo(&self) -> Option<RitmoComposicion> {
        // SAFETY: el dispositivo es propio y sigue vivo; la estructura de
        // salida es local y del tipo que pide la API.
        let e = unsafe { self.dcomp.GetFrameStatistics().ok()? };
        if e.timeFrequency <= 0 {
            return None;
        }
        let a_ms = |t: i64| (t as f64) * 1000.0 / (e.timeFrequency as f64);
        let periodo_ms = if e.currentCompositionRate.Numerator > 0 {
            (e.currentCompositionRate.Denominator as f32) * 1000.0
                / (e.currentCompositionRate.Numerator as f32)
        } else {
            return None;
        };
        // El plazo puede venir en el pasado si la consulta llega tarde: eso
        // no es «falta tiempo negativo», es «ya no da tiempo», o sea 0.
        let hasta_el_plazo_ms =
            (a_ms(e.nextEstimatedFrameTime) - a_ms(e.currentTime)).max(0.0) as f32;
        Some(RitmoComposicion {
            hasta_el_plazo_ms: hasta_el_plazo_ms.min(periodo_ms * 2.0),
            periodo_ms,
        })
    }

    /// El backbuffer ACTUAL envuelto como destino D2D. Hay que volver a
    /// llamarlo en cada fotograma: con flip, el backbuffer rota en cada
    /// present y el bitmap anterior queda apuntando al buffer equivocado.
    pub fn empezar(&self, motor: &MotorRender) -> Result<ID2D1Bitmap1, ErrorRender> {
        // SAFETY: el indice 0 es siempre el backbuffer escribible actual.
        let textura: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0)? };
        motor.destino_backbuffer(&textura)
    }

    /// Cambia el tamano de los buffers sin recrear la composicion. Mucho
    /// mas barato que una `Superficie` nueva (el pin lo hace en cada paso
    /// de un arrastre) y sin el riesgo de que la ventana se quede con la
    /// superficie vieja si la nueva falla.
    pub fn redimensionar(&self, ancho: u32, alto: u32) -> Result<(), ErrorRender> {
        // SAFETY: ningun bitmap del backbuffer sigue vivo fuera de un
        // fotograma (`empezar` lo envuelve de nuevo en cada uno), que es la
        // condicion de ResizeBuffers.
        unsafe {
            self.swapchain.ResizeBuffers(
                0,
                ancho.max(1),
                alto.max(1),
                DXGI_FORMAT_UNKNOWN,
                self.banderas,
            )?
        };
        self.asignado.set((ancho.max(1), alto.max(1)));
        Ok(())
    }

    pub fn presentar(&self) -> Result<(), ErrorRender> {
        // SAFETY: present sin espera de vsync (0,0): el bucle es dirigido
        // por eventos y no debe bloquear el hilo de interfaz.
        unsafe { self.swapchain.Present(0, Default::default()).ok()? };
        Ok(())
    }

    /// Presenta sincronizado con el refresco y, si se sabe, diciendo que
    /// rectangulo cambio (D125). Solo lo usan las ventanas de dibujo: el pin
    /// sigue con `presentar`, que no bloquea su hilo.
    ///
    /// La zona sucia no ahorra pintar (el fotograma se pinta entero sobre la
    /// capa congelada), ahorra componer: DWM solo recompone ese trozo.
    pub fn presentar_sincronizado(
        &self,
        sucio: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), ErrorRender> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Dxgi::{DXGI_PRESENT, DXGI_PRESENT_PARAMETERS};
        let (ancho, alto) = self.asignado.get();
        let mut rect = sucio.map(|(l, t, r, b)| RECT {
            left: l.clamp(0, ancho as i32),
            top: t.clamp(0, alto as i32),
            right: r.clamp(0, ancho as i32),
            bottom: b.clamp(0, alto as i32),
        });
        // Un rectangulo vacio tras recortar no es "nada cambio", es "no se
        // sabe": se presenta entero.
        if rect.is_some_and(|r| r.right <= r.left || r.bottom <= r.top) {
            rect = None;
        }
        let parametros = DXGI_PRESENT_PARAMETERS {
            DirtyRectsCount: u32::from(rect.is_some()),
            pDirtyRects: rect
                .as_mut()
                .map_or(std::ptr::null_mut(), |r| r as *mut RECT),
            pScrollRect: std::ptr::null_mut(),
            pScrollOffset: std::ptr::null_mut(),
        };
        // SAFETY: `parametros` y el rectangulo viven hasta el final de la
        // llamada; la swapchain es de modelo flip, que admite zonas sucias.
        unsafe {
            self.swapchain
                .Present1(1, DXGI_PRESENT(0), &parametros)
                .ok()?
        };
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::{Color, MotorRender};
    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_POPUP,
    };
    use windows::core::w;

    fn d3d() -> ID3D11Device {
        let mut d = None;
        // SAFETY: igual que en motor.rs: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d),
                None,
                None,
            )
            .expect("GPU real");
        }
        d.unwrap()
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio; ejecutar con --ignored"]
    fn se_crea_se_dibuja_y_se_presenta_sin_error() {
        // Una ventana minima descartable. La clase "STATIC" del sistema
        // evita registrar una propia solo para el test.
        // SAFETY: CreateWindowExW con la clase del sistema no exige mas que
        // un modulo valido; la ventana se destruye al final del test.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("prueba"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                256,
                256,
                None,
                None,
                Some(GetModuleHandleW(None).unwrap().into()),
                None,
            )
            .expect("ventana de prueba")
        };

        let d3d = d3d();
        let motor = MotorRender::nuevo(&d3d).unwrap();
        let superficie = Superficie::nueva(&motor, &d3d, hwnd, 256, 256)
            .expect("deberia crearse la superficie de composicion");

        let destino = superficie.empezar(&motor).unwrap();
        let c = motor.contexto();
        // SAFETY: protocolo SetTarget/BeginDraw/EndDraw sobre objetos vivos.
        unsafe {
            c.SetTarget(&destino);
            c.BeginDraw();
            c.Clear(Some(&Color::ACENTO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("present deberia funcionar");

        // Dos fotogramas seguidos: el swapchain rota los buffers y el
        // segundo present fallaria si empezar() no re-envolviera el
        // backbuffer actual. Es el caso negativo del diseno de un solo uso.
        let destino2 = superficie.empezar(&motor).unwrap();
        // SAFETY: igual que arriba.
        unsafe {
            c.SetTarget(&destino2);
            c.BeginDraw();
            c.Clear(Some(&Color::NEGRO.a_d2d()));
            c.EndDraw(None, None).unwrap();
            c.SetTarget(None);
        }
        superficie.presentar().expect("el segundo present tambien");

        drop(superficie);
        // SAFETY: la ventana la creo este test y nadie mas la usa.
        unsafe { DestroyWindow(hwnd).unwrap() };
    }
}
