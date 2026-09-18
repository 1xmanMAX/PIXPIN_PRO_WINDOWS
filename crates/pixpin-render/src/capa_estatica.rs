//! Pintar una vez lo que no se mueve, y copiarlo en cada fotograma.
//!
//! Mientras se arrastra un elemento, los otros 7.999 no cambian. En una iGPU
//! con memoria compartida, pintarlos una vez a un mapa de bits y copiarlo es
//! la diferencia entre arrastrar y **ver** arrastrar.
//!
//! # Solo vive durante un gesto
//!
//! La tentacion es cachear siempre e ir invalidando cuando algo cambia. Eso
//! obliga a saber que ha cambiado en la escena en cada fotograma y acaba en
//! un sistema de invalidacion tan caro como lo que ahorra.
//!
//! Aqui la capa se prepara al pulsar —con todo menos lo seleccionado—, se
//! copia mientras dura el arrastre, y se tira al soltar. Fuera de un gesto
//! no hay nada que invalidar porque no hay capa. Y durante el gesto lo unico
//! que cambia es lo seleccionado, que es justo lo que se excluye.
//!
//! # Lo que cuesta
//!
//! En 1080p son unos 8 MB, y en la maquina suelo la memoria de video sale de
//! los mismos 4 GB. Es **una de las tres copias vivas** que el presupuesto
//! concede al nivel `Ligero`, y solo mientras dura un arrastre. Queda para
//! medir en la tarea 15, no para prometer.

use windows::Win32::Graphics::Direct2D::Common::{
    D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1, ID2D1Bitmap1,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

use crate::lienzo::{Pintor, RectF};
use crate::motor::{ErrorRender, MotorRender};

/// Con que se preparo la capa.
#[derive(Debug, Clone, PartialEq)]
pub struct Estampa {
    /// `x`, `y` y aumento de la camara.
    pub camara: (f32, f32, f32),
    pub tamano: (u32, u32),
    /// Lo que NO esta en la capa porque se esta moviendo.
    pub excluidos: Vec<u64>,
}

/// Si la capa preparada con `preparada` sirve para pintar `ahora`.
///
/// La comparacion de la camara es exacta a proposito. Podria pensarse en una
/// tolerancia —«si se movio menos de un pixel, vale»— pero encuadrar mueve
/// la camara de verdad y la capa esta en coordenadas de pantalla: cualquier
/// desplazamiento la desalinea, y un dibujo medio pixel corrido se ve.
pub fn sigue_valiendo(preparada: &Estampa, ahora: &Estampa) -> bool {
    if preparada.camara != ahora.camara || preparada.tamano != ahora.tamano {
        return false;
    }
    // El orden de la seleccion no significa nada, asi que se comparan como
    // conjuntos: invalidar por reordenarla tiraria la capa sin motivo.
    if preparada.excluidos.len() != ahora.excluidos.len() {
        return false;
    }
    ahora
        .excluidos
        .iter()
        .all(|id| preparada.excluidos.contains(id))
}

/// El mapa de bits de lo que no se mueve durante un gesto.
///
/// Solo vive durante un gesto (ver la documentacion del modulo): fuera de
/// uno, `bitmap` es `None` y no hay nada que ocupe memoria de video ni que
/// invalidar.
pub struct CapaEstatica {
    bitmap: Option<ID2D1Bitmap1>,
    /// Con que se preparo. Vive junto al bitmap: si uno es `None` el otro
    /// tambien, nunca a medias.
    estampa: Option<Estampa>,
}

impl CapaEstatica {
    pub fn nueva() -> Self {
        Self {
            bitmap: None,
            estampa: None,
        }
    }

    /// Prepara la capa: crea un mapa de bits del tamano de `e.tamano`, lo
    /// pone como destino y ejecuta `pintar` sobre el —la escena sin lo
    /// excluido, que decide el llamante— antes de devolver el destino a la
    /// pantalla. Reusa `MotorRender::dibujar` para el protocolo
    /// SetTarget/BeginDraw/EndDraw en vez de repetirlo aqui.
    pub fn preparar(
        &mut self,
        motor: &mut MotorRender,
        e: Estampa,
        pintar: impl FnOnce(&Pintor),
    ) -> Result<(), ErrorRender> {
        let (ancho, alto) = e.tamano;
        let propiedades = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                // Destino de dibujo: como el resto de destinos del crate
                // (ver `motor::envolver`), va premultiplicado.
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: sin datos iniciales (None), D2D solo reserva la memoria;
        // el tamano y el formato de las propiedades son coherentes entre si
        // y el contexto esta vivo (lo mantiene `motor`).
        let bitmap = unsafe {
            motor.contexto().CreateBitmap(
                D2D_SIZE_U {
                    width: ancho,
                    height: alto,
                },
                None,
                0,
                &propiedades,
            )?
        };
        motor.dibujar(&bitmap, pintar)?;
        self.bitmap = Some(bitmap);
        self.estampa = Some(e);
        Ok(())
    }

    /// Si la capa vale para `ahora`, copia el mapa de bits sobre `destino` y
    /// devuelve `true`. Si no vale, no toca `destino` y devuelve `false`:
    /// quien llama pinta todo como siempre. Nunca repinta la capa aqui —eso
    /// solo lo hace `preparar`, al principio del gesto.
    pub fn volcar(&self, motor: &mut MotorRender, destino: &ID2D1Bitmap1, ahora: &Estampa) -> bool {
        self.volcar_zona(motor, destino, ahora, None)
    }

    /// Lo mismo, pero copiando SOLO un trozo.
    ///
    /// Copiar la pantalla entera sesenta veces por segundo es lo que hacia
    /// lenta la tinta: en el equipo del usuario eran 6-8 ms por fotograma
    /// —medidos— antes de dibujar nada, y en una grafica integrada eso es
    /// medio fotograma tirado en mover pixeles que no cambian.
    ///
    /// Quien llama tiene que pasar la union de la zona sucia de ESTE
    /// fotograma y la del anterior: la cadena de intercambio tiene dos
    /// mapas, asi que el que se pinta ahora lleva dentro lo de hace DOS
    /// fotogramas. Con solo la zona de este quedaria pegada la punta del
    /// trazo de hace dos, que es el fantasma clasico de esta tecnica.
    pub fn volcar_zona(
        &self,
        motor: &mut MotorRender,
        destino: &ID2D1Bitmap1,
        ahora: &Estampa,
        zona: Option<(i32, i32, i32, i32)>,
    ) -> bool {
        let (Some(bitmap), Some(preparada)) = (&self.bitmap, &self.estampa) else {
            return false;
        };
        if !sigue_valiendo(preparada, ahora) {
            return false;
        }
        let (ancho, alto) = ahora.tamano;
        let entera = RectF {
            x: 0.0,
            y: 0.0,
            ancho: ancho as f32,
            alto: alto as f32,
        };
        let caja = match zona {
            None => entera,
            Some((x0, y0, x1, y1)) => {
                let x0 = x0.clamp(0, ancho as i32) as f32;
                let y0 = y0.clamp(0, alto as i32) as f32;
                let x1 = x1.clamp(0, ancho as i32) as f32;
                let y1 = y1.clamp(0, alto as i32) as f32;
                // Una zona vacia tras recortar no es «no cambio nada», es
                // «no se sabe»: se copia entera, como antes.
                if x1 <= x0 || y1 <= y0 {
                    entera
                } else {
                    RectF {
                        x: x0,
                        y: y0,
                        ancho: x1 - x0,
                        alto: y1 - y0,
                    }
                }
            }
        };
        motor
            .dibujar(destino, |p| {
                // Mismo rectangulo en origen y en destino: es una copia, no
                // un escalado, asi que no hay interpolacion que valorar.
                p.bitmap(bitmap, caja, Some(caja), true);
            })
            .is_ok()
    }

    /// Tira el mapa de bits de verdad (no lo marca como invalido): si no, los
    /// 8 MB se quedan vivos entre gestos y el presupuesto de copias vivas se
    /// incumple en reposo, que es donde mas duele.
    pub fn soltar(&mut self) {
        self.bitmap = None;
        self.estampa = None;
    }

    pub fn lista(&self) -> bool {
        self.bitmap.is_some()
    }

    /// Cuanto ocupa el mapa de bits ahora mismo. Cero si no hay gesto en
    /// curso.
    pub fn bytes(&self) -> usize {
        match &self.estampa {
            Some(e) if self.bitmap.is_some() => e.tamano.0 as usize * e.tamano.1 as usize * 4,
            _ => 0,
        }
    }
}

impl Default for CapaEstatica {
    fn default() -> Self {
        Self::nueva()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn estampa() -> Estampa {
        Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (1920, 1080),
            excluidos: vec![7],
        }
    }

    #[test]
    fn la_misma_estampa_vale() {
        assert!(sigue_valiendo(&estampa(), &estampa()));
    }

    #[test]
    fn mover_la_camara_la_invalida() {
        // La capa esta pintada en coordenadas de pantalla: si el lienzo se
        // desplaza, lo pintado ya no cae donde toca.
        let mut ahora = estampa();
        ahora.camara.0 += 1.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_aumento_la_invalida() {
        let mut ahora = estampa();
        ahora.camara.2 = 2.0;
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_el_tamano_de_la_ventana_la_invalida() {
        let mut ahora = estampa();
        ahora.tamano = (1280, 720);
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn cambiar_lo_excluido_la_invalida() {
        // Si la seleccion cambia a mitad de gesto, la capa lleva pintado un
        // elemento que ahora se esta moviendo: se veria por duplicado, uno
        // quieto y otro siguiendo al raton.
        let mut ahora = estampa();
        ahora.excluidos = vec![8];
        assert!(!sigue_valiendo(&estampa(), &ahora));
    }

    #[test]
    fn el_orden_de_lo_excluido_no_importa() {
        // La seleccion es un Vec cuyo orden no significa nada. Invalidar
        // por reordenarlo tiraria la capa sin motivo.
        let preparada = Estampa {
            excluidos: vec![1, 2, 3],
            ..estampa()
        };
        let ahora = Estampa {
            excluidos: vec![3, 1, 2],
            ..estampa()
        };
        assert!(sigue_valiendo(&preparada, &ahora));
    }

    #[test]
    fn una_capa_recien_creada_no_esta_lista() {
        let c = CapaEstatica::nueva();
        assert!(!c.lista());
        assert_eq!(c.bytes(), 0, "sin gesto en curso no ocupa nada");
    }

    // A partir de aqui, lo que toca GPU: necesita un dispositivo Direct3D y
    // windows-latest no lo ofrece de forma fiable (la misma razon que las
    // pruebas de bandeja y de atajos van marcadas #[ignore]).

    use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
        D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;

    use crate::motor::Color;

    fn dispositivo() -> (ID3D11Device, ID3D11DeviceContext) {
        let mut d3d = None;
        let mut ctx = None;
        // SAFETY: salidas locales, constantes documentadas.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                Default::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut ctx),
            )
            .expect("GPU real");
        }
        (d3d.unwrap(), ctx.unwrap())
    }

    fn textura(d3d: &ID3D11Device, ancho: u32, alto: u32) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: ancho,
            Height: alto,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            ..Default::default()
        };
        let mut t = None;
        // SAFETY: desc inicializada; t es la salida local.
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.unwrap();
        t.unwrap()
    }

    fn pixel(
        d3d: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        t: &ID3D11Texture2D,
        x: u32,
        y: u32,
    ) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: GetDesc rellena la estructura local.
        unsafe { t.GetDesc(&mut desc) };
        let e_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
            ..desc
        };
        let mut e = None;
        // SAFETY: desc inicializada; e es la salida local.
        unsafe { d3d.CreateTexture2D(&e_desc, None, Some(&mut e)) }.unwrap();
        let e = e.unwrap();
        // SAFETY: mismo formato/tamano; staging legible tras CopyResource.
        unsafe { ctx.CopyResource(&e, t) };
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: staging con lectura; se desmapea justo despues.
        unsafe { ctx.Map(&e, 0, D3D11_MAP_READ, 0, Some(&mut mapa)) }.unwrap();
        // SAFETY: (x, y) dentro de la textura, obligacion del llamante.
        let v = unsafe {
            let base =
                (mapa.pData as *const u8).add(y as usize * mapa.RowPitch as usize + x as usize * 4);
            [*base, *base.add(1), *base.add(2), *base.add(3)]
        };
        // SAFETY: empareja el Map de arriba.
        unsafe { ctx.Unmap(&e, 0) };
        v
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn preparar_pinta_la_capa_y_volcar_la_copia_sobre_el_destino() {
        let (d3d, ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        let mut capa = CapaEstatica::nueva();
        assert!(!capa.lista());

        let e = Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        capa.preparar(&mut motor, e.clone(), |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        })
        .expect("deberia prepararse la capa");
        assert!(capa.lista());
        assert_eq!(capa.bytes(), 64 * 64 * 4);

        // Limpia el destino real a azul antes de volcar: si volcar no
        // copiara nada, el destino seguiria azul.
        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
            })
            .unwrap();

        let volco = capa.volcar(&mut motor, &destino, &e);
        assert!(volco, "la misma estampa deberia seguir valiendo");
        // BGRA: el rojo de la capa quedo copiado sobre el destino.
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 32, 32), [0, 0, 255, 255]);

        capa.soltar();
        assert!(!capa.lista(), "soltar tiene que dejar caer el bitmap");
        assert_eq!(capa.bytes(), 0, "sin capa no hay memoria ocupada");
    }

    #[test]
    #[ignore = "necesita GPU real; ejecutar con --ignored"]
    fn volcar_no_copia_nada_si_la_estampa_ya_no_vale() {
        let (d3d, ctx) = dispositivo();
        let mut motor = MotorRender::nuevo(&d3d).unwrap();
        let destino_tex = textura(&d3d, 64, 64);
        let destino = motor.destino_desde_textura(&destino_tex).unwrap();

        let mut capa = CapaEstatica::nueva();
        let preparada = Estampa {
            camara: (0.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        capa.preparar(&mut motor, preparada, |p| {
            p.limpiar(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        })
        .unwrap();

        motor
            .dibujar(&destino, |p| {
                p.limpiar(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                });
            })
            .unwrap();

        // La camara se movio a mitad de gesto (p.ej. se encuadro): la
        // estampa ya no vale y volcar no debe tocar el destino.
        let ahora = Estampa {
            camara: (5.0, 0.0, 1.0),
            tamano: (64, 64),
            excluidos: vec![],
        };
        let volco = capa.volcar(&mut motor, &destino, &ahora);
        assert!(!volco, "la camara se movio: la capa no vale");
        assert_eq!(pixel(&d3d, &ctx, &destino_tex, 32, 32), [255, 0, 0, 255]);
    }
}
