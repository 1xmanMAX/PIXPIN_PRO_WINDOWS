//! Pintar la tinta como Excalidraw: contorno cerrado, curvas cuadraticas
//! por puntos medios y relleno *winding*.
//!
//! `pasos_de_tinta` es `getSvgPathFromStroke` de Excalidraw
//! (`packages/element/src/shape.ts` @afa3a65, MIT) sin el recorte a dos
//! decimales, que solo existe para acortar el SVG. Es pura: se prueba sin
//! GPU contra el oraculo.
//!
//! Por que *winding* y no la regla por defecto de Direct2D (alternada): el
//! contorno de perfect-freehand se cruza consigo mismo en bucles, retrocesos
//! y en los arcos de las esquinas. Con la alternada cada cruce abre un
//! agujero; Canvas2D rellena con *nonzero*, que es *winding*. `Pintor::velo`
//! SI necesita la alternada para su hueco, por eso esto es una geometria
//! aparte y no un cambio en `Pintor::geometria`.

use std::collections::HashMap;
use std::time::Duration;

use windows::Win32::Graphics::Direct2D::ID2D1GeometryRealization;

/// Cuanto tiene que estar quieto el zoom para rehacer la tinta nitida (D122,
/// el `shouldCacheIgnoreZoom` de Excalidraw). Mientras tanto se estira lo
/// realizado: algo borroso y casi gratis.
pub fn retardo_nitido(nivel: pixpin_nivel::Nivel) -> Duration {
    match nivel {
        pixpin_nivel::Nivel::Ligero => Duration::from_millis(500),
        _ => Duration::from_millis(300),
    }
}

pub(crate) struct Realizada {
    pub(crate) version: u32,
    pub(crate) realizacion: ID2D1GeometryRealization,
    /// El fotograma en que se pinto por ultima vez: lo que decide a quien se
    /// echa cuando la cache se pasa de `MAX_REALIZACIONES`.
    pub(crate) usado: u64,
}

/// Cuantas realizaciones se guardan a la vez.
///
/// Generoso a proposito: una escena de 2.000 formas de rough.js con relleno
/// de sombreado puede pedir decenas de miles de ordenes, y echar lo que el
/// siguiente fotograma va a volver a pedir seria peor que no cachear. El
/// tope existe solo para que un documento enorme no se coma la memoria de
/// video de una HD 4000; al pasarse se echa primero lo que no se pinto ni en
/// este fotograma ni en el anterior.
const MAX_REALIZACIONES: usize = 60_000;

/// La geometria ya teselada por Direct2D, por elemento y orden.
///
/// Una realizacion es la geometria convertida en triangulos a una escala:
/// pintarla no recalcula nada en el procesador. Se rehace solo si cambia la
/// version del elemento o si se fija otra escala.
///
/// Empezo guardando solo la tinta (D121) y ahora guarda TODAS las ordenes
/// de geometria: los poligonos, los rellenos y las polilineas de rough.js
/// tambien. Sin eso, un fotograma de paneo creaba una `ID2D1PathGeometry`
/// por cada pasada de cada borde y por cada raya de cada sombreado —de 20 a
/// 60 por forma rellena— y las tiraba al acabar.
pub struct CacheTinta {
    pub(crate) mapa: HashMap<(u64, u32), Realizada>,
    pub(crate) escala: f32,
    /// **La tela con la que se tine la silueta**: las brochas de mosaico de
    /// `Pintor::grano`, una por material y color.
    ///
    /// Vive dentro y no al lado por una razon practica: va exactamente a los
    /// mismos sitios que la silueta —quien pinta un trazo pinta su grano— y
    /// las dos son recursos del MISMO dispositivo Direct2D, asi que se
    /// pierden y se vacian a la vez. Tenerlas separadas obligaba a llevar un
    /// segundo parametro por las cinco funciones de pintado de la ventana
    /// para no perderlo jamas de vista.
    pub grano: crate::grano::CacheGrano,
}

impl CacheTinta {
    pub fn nueva() -> Self {
        Self {
            mapa: HashMap::new(),
            escala: 1.0,
            grano: crate::grano::CacheGrano::nueva(),
        }
    }

    /// Dispositivo perdido o documento nuevo: las realizaciones son del
    /// dispositivo viejo y no valen. **Las brochas del grano tampoco**: son
    /// bitmaps del mismo dispositivo, y sobrevivir a su muerte es justo el
    /// fallo que se ve como un lienzo en blanco.
    pub fn vaciar(&mut self) {
        self.mapa.clear();
        self.grano.vaciar();
    }

    pub fn escala(&self) -> f32 {
        self.escala
    }

    /// Se llama cuando el zoom lleva `retardo_nitido` quieto.
    pub fn fijar_escala(&mut self, escala: f32) {
        if escala != self.escala {
            self.escala = escala;
            self.mapa.clear();
        }
    }

    pub fn cuantas(&self) -> usize {
        self.mapa.len()
    }

    /// Si ya hay una realizacion valida para `clave` (mismo elemento, mismo
    /// indice de orden, misma version). Quien pinta la usa para decidir SI
    /// hace falta reconstruir el contorno en `Vec<(f32, f32)>` -en un
    /// acierto no hace falta, y esa reconstruccion es la que se paga cada
    /// fotograma para nada si no se consulta antes-.
    pub fn vale(&self, clave: (u64, u32, u32)) -> bool {
        let (id, version, indice) = clave;
        self.mapa
            .get(&(id, indice))
            .is_some_and(|r| r.version == version)
    }

    /// Guarda una realizacion recien hecha, echando lo viejo si hace falta.
    /// `fotograma` es el de `MotorRender`, que ya cuenta uno por `dibujar`.
    pub(crate) fn guardar(
        &mut self,
        clave: (u64, u32, u32),
        fotograma: u64,
        realizacion: ID2D1GeometryRealization,
    ) {
        let (id, version, indice) = clave;
        if self.mapa.len() >= MAX_REALIZACIONES {
            self.mapa.retain(|_, r| r.usado + 1 >= fotograma);
            if self.mapa.len() >= MAX_REALIZACIONES {
                self.mapa.clear();
            }
        }
        self.mapa.insert(
            (id, indice),
            Realizada {
                version,
                realizacion,
                usado: fotograma,
            },
        );
    }

    /// Apunta que `clave` se pinto en `fotograma`, para que la limpieza no
    /// la eche. Devuelve la realizacion si la hay y es de esa version.
    pub(crate) fn tomar(
        &mut self,
        clave: (u64, u32, u32),
        fotograma: u64,
    ) -> Option<ID2D1GeometryRealization> {
        let (id, version, indice) = clave;
        let r = self.mapa.get_mut(&(id, indice))?;
        if r.version != version {
            return None;
        }
        r.usado = fotograma;
        Some(r.realizacion.clone())
    }
}

impl Default for CacheTinta {
    fn default() -> Self {
        Self::nueva()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PasoTrayecto {
    Mover((f32, f32)),
    Cuadratica {
        control: (f32, f32),
        fin: (f32, f32),
    },
    Linea((f32, f32)),
    Cerrar,
}

/// `M p0 Q p0 m01 p1 m12 ... pN mN0 L p0 Z`.
pub fn pasos_de_tinta(contorno: &[(f32, f32)]) -> Vec<PasoTrayecto> {
    let Some(&primero) = contorno.first() else {
        return Vec::new();
    };
    let medio = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let max = contorno.len() - 1;
    let mut pasos = Vec::with_capacity(contorno.len() + 3);
    pasos.push(PasoTrayecto::Mover(primero));
    for (i, &p) in contorno.iter().enumerate() {
        let siguiente = if i == max { primero } else { contorno[i + 1] };
        pasos.push(PasoTrayecto::Cuadratica {
            control: p,
            fin: medio(p, siguiente),
        });
    }
    pasos.push(PasoTrayecto::Linea(primero));
    pasos.push(PasoTrayecto::Cerrar);
    pasos
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::motor::Color;

    #[test]
    fn el_retardo_nitido_es_mas_largo_en_ligero() {
        use pixpin_nivel::Nivel;
        assert_eq!(retardo_nitido(Nivel::Completo).as_millis(), 300);
        assert_eq!(retardo_nitido(Nivel::Ligero).as_millis(), 500);
    }

    #[test]
    fn fijar_otra_escala_tira_lo_realizado_y_la_misma_no() {
        let mut c = CacheTinta::nueva();
        assert_eq!(c.escala(), 1.0);
        c.fijar_escala(1.0);
        assert_eq!(c.escala(), 1.0);
        c.fijar_escala(2.0);
        assert_eq!(c.escala(), 2.0);
        assert_eq!(c.cuantas(), 0);
    }

    #[test]
    fn una_cache_vacia_no_vale_para_ninguna_clave() {
        let c = CacheTinta::nueva();
        assert!(!c.vale((1, 1, 0)));
    }

    /// Motor y un destino de `ancho x alto` sobre un D3D11 hardware propio.
    fn motor_y_destino_de_prueba(
        ancho: u32,
        alto: u32,
    ) -> (
        crate::MotorRender,
        windows::Win32::Graphics::Direct2D::ID2D1Bitmap1,
    ) {
        use windows::Win32::Foundation::HMODULE;
        use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11CreateDevice,
        };
        use windows::Win32::Graphics::Dxgi::Common::{
            DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
        };
        let mut d3d = None;
        // SAFETY: salidas locales; sin adaptador concreto ni capas de depuracion.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                None,
            )
            .expect("sin D3D11 hardware");
        }
        let d3d = d3d.expect("dispositivo");
        let motor = crate::MotorRender::nuevo(&d3d).expect("motor");
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
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            ..Default::default()
        };
        let mut textura = None;
        // SAFETY: descripcion local valida; salida local.
        unsafe {
            d3d.CreateTexture2D(&desc, None, Some(&mut textura))
                .expect("textura")
        };
        let destino = motor
            .destino_desde_textura(&textura.expect("textura"))
            .expect("destino");
        (motor, destino)
    }

    #[test]
    #[ignore = "necesita GPU y sesion de escritorio"]
    fn pintar_dos_veces_la_misma_clave_realiza_una_sola_vez() {
        let (motor, destino) = motor_y_destino_de_prueba(64, 64);
        let mut cache = CacheTinta::nueva();
        let contorno = [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)];
        assert!(!cache.vale((7, 1, 0)), "todavia no se ha pintado nada");
        for _ in 0..2 {
            motor
                .dibujar(&destino, |p| {
                    p.tinta_cacheada(&mut cache, (7, 1, 0), &contorno, Color::NEGRO)
                })
                .unwrap();
        }
        assert_eq!(cache.cuantas(), 1);
        assert!(cache.vale((7, 1, 0)), "la version 1 quedo realizada");
        assert!(!cache.vale((7, 2, 0)), "la version 2 no se ha pintado aun");
        motor
            .dibujar(&destino, |p| {
                p.tinta_cacheada(&mut cache, (7, 2, 0), &contorno, Color::NEGRO)
            })
            .unwrap();
        assert_eq!(
            cache.cuantas(),
            1,
            "una version nueva sustituye, no acumula"
        );
    }

    #[test]
    fn un_contorno_vacio_no_da_ningun_paso() {
        assert!(pasos_de_tinta(&[]).is_empty());
    }

    #[test]
    fn cada_vertice_es_el_control_de_una_cuadratica_que_acaba_en_el_punto_medio() {
        let c = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        assert_eq!(
            pasos_de_tinta(&c),
            vec![
                PasoTrayecto::Mover((0.0, 0.0)),
                PasoTrayecto::Cuadratica {
                    control: (0.0, 0.0),
                    fin: (5.0, 0.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 0.0),
                    fin: (10.0, 5.0)
                },
                PasoTrayecto::Cuadratica {
                    control: (10.0, 10.0),
                    fin: (5.0, 5.0)
                },
                PasoTrayecto::Linea((0.0, 0.0)),
                PasoTrayecto::Cerrar,
            ]
        );
    }

    /// Los numeros del SVG de Excalidraw, en orden, sin las letras.
    fn numeros_svg(svg: &str) -> Vec<f32> {
        svg.split_whitespace()
            .filter(|t| !t.chars().all(|c| c.is_ascii_alphabetic()))
            .flat_map(|t| {
                t.split(',')
                    .map(|n| n.parse::<f32>().expect("numero"))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn numeros_pasos(p: &[PasoTrayecto]) -> Vec<f32> {
        p.iter()
            .flat_map(|paso| match *paso {
                PasoTrayecto::Mover(a) | PasoTrayecto::Linea(a) => vec![a.0, a.1],
                PasoTrayecto::Cuadratica { control, fin } => {
                    vec![control.0, control.1, fin.0, fin.1]
                }
                PasoTrayecto::Cerrar => vec![],
            })
            .collect()
    }

    #[test]
    fn el_trayecto_coincide_con_get_svg_path_from_stroke_de_excalidraw() {
        // NOTA (ruling del controlador): se compara contra `svg_crudo`, no
        // `svg`. El regex de truncado de Excalidraw a dos decimales corrompe
        // numeros en notacion exponencial (1.2e-16 pasa a "1.22"), lo que
        // daria un rojo falso. `svg_crudo` puede traer esa notacion
        // ("1.2246467991473532e-16"); f32::parse la acepta sin problema.
        let ruta = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pixpin-motor2d/tests/oraculo/tinta.json"
        );
        let texto = std::fs::read_to_string(ruta).expect("falta el oraculo (Tarea 1)");
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        for caso in v["casos"].as_array().unwrap() {
            let contorno: Vec<(f32, f32)> = caso["contorno"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| (p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32))
                .collect();
            let esperado = numeros_svg(caso["svg_crudo"].as_str().unwrap());
            let obtenido = numeros_pasos(&pasos_de_tinta(&contorno));
            assert_eq!(esperado.len(), obtenido.len(), "{}", caso["nombre"]);
            for (e, o) in esperado.iter().zip(&obtenido) {
                // Coordenadas de pixel: la conversion f64->f32 se queda muy
                // por debajo de esta tolerancia.
                assert!((e - o).abs() <= 0.001, "{}: {e} contra {o}", caso["nombre"]);
            }
        }
    }
}
