//! **Cuanto tarda abrir una imagen como pin**, medido.
//!
//! PixPin es la aplicacion predeterminada de imagenes: un doble clic en el
//! Explorador lanza `pixpinmax.exe <ruta>`, que pasa la ruta a la copia viva
//! y se va (`main::arrancar`, antes de leer ni un ajuste). Lo que tarda de
//! verdad es pinearla en la copia viva: el camino de antes y el de ahora
//! (`pines::foto`) se miden aqui, por tramos, con fotos de verdad.

#[cfg(test)]
mod medir {
    //! Las mediciones de abrir imagenes (`cargo test -p pixpin --bin
    //! pixpinmax medir_apertura -- --ignored --nocapture`). Abren pines de
    //! verdad un instante y los cierran.
    //!
    //! Las fotos: `PIXPIN_MEDIR_FOTOS` (rutas separadas por `;`). Solo se
    //! leen; el almacen es una carpeta temporal.

    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use crate::overlay::Recursos;
    use crate::pines::Pines;
    use pixpin_store::Catalogo;

    pub(super) fn fotos() -> Vec<PathBuf> {
        std::env::var("PIXPIN_MEDIR_FOTOS")
            .map(|v| {
                v.split(';')
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn mediana(mut v: Vec<Duration>) -> f64 {
        v.sort();
        v[v.len() / 2].as_secs_f64() * 1000.0
    }

    /// Memoria privada y conjunto de trabajo del proceso, en megas.
    pub(super) fn memoria() -> (f64, f64) {
        let pid = std::process::id();
        let orden =
            format!("$p=Get-Process -Id {pid}; \"$($p.PrivateMemorySize64) $($p.WorkingSet64)\"");
        let Ok(s) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &orden])
            .output()
        else {
            return (0.0, 0.0);
        };
        let texto = String::from_utf8_lossy(&s.stdout);
        let mut n = texto
            .split_whitespace()
            .map(|x| x.parse::<f64>().unwrap_or(0.0) / (1024.0 * 1024.0));
        (n.next().unwrap_or(0.0), n.next().unwrap_or(0.0))
    }

    pub(super) fn bombear(ms: u64) {
        let hasta = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < hasta {
            pixpin_shell::overlay::bombear_pendientes();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn gestor(etiqueta: &str) -> (Recursos, Pines, pixpin_geom::Monitor) {
        let dir = std::env::temp_dir().join(format!("pixpin-medir-{etiqueta}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let textos = Catalogo::nuevo(pixpin_store::Idioma::Espanol);
        let r = Recursos::nuevos().unwrap();
        let p = Pines::nuevos(
            &dir,
            r.d3d(),
            r.motor(),
            String::new(),
            crate::textos_del_pin(&textos),
            String::new(),
            String::new(),
            windows::Win32::Foundation::HWND::default(),
            Some(16),
        )
        .unwrap();
        let d = pixpin_capture::enumerar_monitores().unwrap();
        let m = d.principal().unwrap().to_owned();
        (r, p, m)
    }

    /// El camino de ANTES (el de `pinear_portapapeles` hasta el 4-oct):
    /// descodificar entera, copiar, recodificar a PNG para el almacen y subir
    /// la imagen entera a la GPU. Por tramos.
    #[test]
    #[ignore = "abre pines de verdad; necesita GPU y PIXPIN_MEDIR_FOTOS"]
    fn medir_apertura_antes() {
        let (r, mut p, m) = gestor("antes");
        let motor = r.motor();
        for ruta in fotos() {
            let mut t = [(); 6].map(|_| Vec::new());
            let mut dims = (0, 0);
            for _ in 0..10 {
                let t0 = Instant::now();
                let img = pixpin_codec::cargar(&ruta).unwrap();
                t[0].push(t0.elapsed());
                dims = (img.ancho, img.alto);
                let t1 = Instant::now();
                let copia = img.clone();
                t[1].push(t1.elapsed());
                drop(copia);
                let t2 = Instant::now();
                let png = pixpin_codec::codificar_png(&img).unwrap();
                t[2].push(t2.elapsed());
                drop(png);
                let t3 = Instant::now();
                let b = motor
                    .bitmap_desde_pixeles(img.ancho, img.alto, &img.pixeles)
                    .unwrap();
                t[3].push(t3.elapsed());
                drop(b);
                // De punta a punta, como lo hacia la app.
                let t4 = Instant::now();
                let img = pixpin_codec::cargar(&ruta).unwrap();
                p.pinear_imagen_centrada(&img, &m).unwrap();
                t[4].push(t4.elapsed());
                pixpin_shell::overlay::esperar_composicion();
                t[5].push(t4.elapsed());
                bombear(30);
                p.cerrar_todos();
                p.purgar();
            }
            println!(
                "ANTES {} {}x{}: descodificar {:.1} ms, copiar {:.1}, png {:.1}, subir {:.1}, \
                 TOTAL hasta el pin {:.1} ms, en pantalla {:.1} ms",
                ruta.file_name().unwrap().to_string_lossy(),
                dims.0,
                dims.1,
                mediana(t[0].clone()),
                mediana(t[1].clone()),
                mediana(t[2].clone()),
                mediana(t[3].clone()),
                mediana(t[4].clone()),
                mediana(t[5].clone()),
            );
        }
        // Memoria tras abrir cinco fotos (las de la lista, hasta cinco).
        {
            bombear(200);
            let (antes_p, antes_w) = memoria();
            for ruta in fotos().iter().cycle().take(5) {
                let img = pixpin_codec::cargar(ruta).unwrap();
                p.pinear_imagen_centrada(&img, &m).unwrap();
            }
            bombear(500);
            let (p5, w5) = memoria();
            println!(
                "ANTES memoria con 5 fotos abiertas: privada {p5:.0} MB (+{:.0}), trabajo {w5:.0} MB (+{:.0})",
                p5 - antes_p,
                w5 - antes_w
            );
            p.cerrar_todos();
            p.purgar();
        }
    }

    /// El camino de AHORA (`Pines::pinear_foto`): leida a la medida del pin
    /// con WIC y ya derecha, copiada tal cual al almacen, subida reducida.
    #[test]
    #[ignore = "abre pines de verdad; necesita GPU y PIXPIN_MEDIR_FOTOS"]
    fn medir_apertura_ahora() {
        // El desglose de cada apertura sale en el registro ("foto pineada").
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
        let (_r, mut p, m) = gestor("ahora");
        let caja = crate::pines::foto::caja_de_lectura(&m);
        for ruta in fotos() {
            let mut t = [(); 4].map(|_| Vec::new());
            let mut dims = ((0, 0), (0, 0));
            let mut con_previa = false;
            for _ in 0..10 {
                let t0 = Instant::now();
                let v = pixpin_codec::vista::cargar_para_ver(&ruta, caja).unwrap();
                t[0].push(t0.elapsed());
                dims = (
                    (v.ancho_completo, v.alto_completo),
                    (v.imagen.ancho, v.imagen.alto),
                );
                drop(v);
                let t1 = Instant::now();
                p.pinear_foto(&ruta, &m).unwrap();
                t[1].push(t1.elapsed());
                pixpin_shell::overlay::esperar_composicion();
                t[2].push(t1.elapsed());
                // La buena, si salio antes la vista previa.
                con_previa = p.esperar_foto_leida(Duration::from_secs(3));
                t[3].push(t1.elapsed());
                bombear(30);
                p.cerrar_todos();
                p.purgar();
            }
            println!(
                "AHORA {} {}x{} (leida {}x{}): leer {:.1} ms, TOTAL hasta el pin {:.1} ms, \
                 en pantalla {:.1} ms{}",
                ruta.file_name().unwrap().to_string_lossy(),
                dims.0.0,
                dims.0.1,
                dims.1.0,
                dims.1.1,
                mediana(t[0].clone()),
                mediana(t[1].clone()),
                mediana(t[2].clone()),
                if con_previa {
                    format!(
                        " (con vista previa; afinada a los {:.1} ms)",
                        mediana(t[3].clone())
                    )
                } else {
                    String::new()
                },
            );
        }
        {
            bombear(200);
            let (antes_p, antes_w) = memoria();
            for ruta in fotos().iter().cycle().take(5) {
                p.pinear_foto(ruta, &m).unwrap();
            }
            // Con las buenas ya puestas: es lo que queda en memoria.
            while p.esperar_foto_leida(Duration::from_secs(3)) {}
            bombear(500);
            let (p5, w5) = memoria();
            println!(
                "AHORA memoria con 5 fotos abiertas: privada {p5:.0} MB (+{:.0}), trabajo {w5:.0} MB (+{:.0})",
                p5 - antes_p,
                w5 - antes_w
            );
            p.cerrar_todos();
            p.purgar();
        }
    }
}
