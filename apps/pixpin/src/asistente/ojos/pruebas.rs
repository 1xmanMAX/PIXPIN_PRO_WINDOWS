//! Los ojos pintados de verdad (GPU o WARP), sin ventana.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use pixpin_ia::edicion::Contexto;
use pixpin_ia::ojos::{Mirada, Zona};
use pixpin_motor2d::Escena;

use super::*;

/// Una escena hecha como la hace el asistente: una llamada a
/// `dibujar_en_lienzo` → Excalidraw → el lienzo.
pub(crate) fn escena_de(json: &str) -> Escena {
    let cruda = pixpin_ia::motor::LlamadaCruda { id: "x".into(), nombre: "dibujar_en_lienzo".into(), argumentos: json.into() };
    let v = pixpin_ia::herramientas::validar(&cruda, &pixpin_ia::herramientas::catalogo()).expect("llamada valida");
    let doc = pixpin_ia::lienzo::documento(pixpin_ia::lienzo::elementos(&v.llamada, (0.0, 0.0), 7));
    let mut escena = Escena::nueva();
    for e in crate::asistente::al_lienzo::elementos_de(&doc).unwrap() {
        escena.anadir(e);
    }
    escena
}

pub(crate) const TRES_FIGURAS: &str = r#"{"elementos":[
  {"tipo":"rectangulo","caja":[60,80,200,120],"color":"azul","relleno":"azul"},
  {"tipo":"elipse","caja":[340,80,160,160],"color":"rojo"},
  {"tipo":"triangulo","caja":[560,90,160,140],"color":"verde"},
  {"tipo":"texto","x":360,"y":150,"texto":"Etiqueta encima del circulo"}]}"#;

fn salida() -> Option<PathBuf> {
    std::env::var_os("PIXPIN_OJOS_SALIDA").map(PathBuf::from)
}

fn leer_png(png: &[u8]) -> pixpin_codec::ImagenRgba {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let r = std::env::temp_dir().join(format!("pixpin-ojos-{}-{}.png", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    std::fs::write(&r, png).unwrap();
    let img = pixpin_codec::imagen::cargar(&r).unwrap();
    let _ = std::fs::remove_file(&r);
    img
}

fn pixel(img: &pixpin_codec::ImagenRgba, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.ancho + x) * 4) as usize;
    [img.pixeles[i], img.pixeles[i + 1], img.pixeles[i + 2], img.pixeles[i + 3]]
}

#[test]
fn pinta_el_lienzo_a_un_png_pequeno_con_rejilla_y_etiquetas_rapido() {
    let escena = escena_de(TRES_FIGURAS);
    let ctx = Contexto { marco: pixpin_ia::edicion::marco_de(&escena), ..Default::default() };
    let enc = encargo_de(&escena, &Mirada::default(), &ctx, |_| None).unwrap();
    assert_eq!(enc.escena.elementos.len(), escena.cuantos_visibles());
    // La primera crea la GPU; la que cuenta es la siguiente.
    let t0 = Instant::now();
    let v = pintar(enc.clone()).unwrap();
    let primera = t0.elapsed().as_millis();
    let t1 = Instant::now();
    let v2 = pintar(enc.clone()).unwrap();
    let segunda = t1.elapsed().as_millis();
    println!("vista: {}x{} {} KB; primera {primera} ms, segunda {segunda} ms (pintar {} ms)", v.ancho, v.alto, v.png.len() / 1024, v2.ms);
    assert!(v.ancho.max(v.alto) <= pixpin_ia::ojos::LADO_MAXIMO);
    assert!(v.png.len() < 200 * 1024, "pocos KB: {}", v.png.len());
    assert!(segunda < if cfg!(debug_assertions) { 2000 } else { 200 }, "una vista con la GPU ya lista: {segunda} ms");
    assert!(v.texto.starts_with(pixpin_ia::ojos::MARCA) && v.texto.contains("#id"));
    // El relleno azul del rectangulo esta donde dice la caja.
    let img = leer_png(&v.png);
    let (x0, y0, _, _) = enc.caja;
    let (_, _, s) = pixpin_ia::ojos::tamano(enc.caja, pixpin_ia::ojos::LADO_MAXIMO);
    let rect = escena.visibles().next().unwrap().caja();
    let c = pixel(&img, (((rect.0 + rect.2) / 2.0 - x0) * s) as u32, (((rect.1 + rect.3) / 2.0 + 25.0 - y0) * s) as u32);
    assert!(c[2] > c[0] + 40, "azul en el centro del rectangulo: {c:?}");
    // Un rincon vacio es papel (casi blanco).
    let r = pixel(&img, img.ancho - 2, img.alto - 2);
    assert!(r[0] > 200 && r[1] > 200 && r[2] > 200, "{r:?}");
    if let Some(d) = salida() {
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("vista-tres-figuras.png"), &v.png).unwrap();
    }
}

#[test]
fn una_zona_pequena_se_amplia_y_solo_copia_lo_suyo() {
    let escena = escena_de(TRES_FIGURAS);
    let ctx = Contexto { marco: (0.0, 0.0), ..Default::default() };
    let id_rect = escena.visibles().next().unwrap().id;
    let id_circulo = escena.visibles().nth(2).unwrap().id;
    let enc = encargo_de(&escena, &Mirada::de(Zona::Elementos(format!("#{id_circulo}"))), &ctx, |_| None).unwrap();
    assert!(enc.escena.elementos.len() < escena.cuantos_visibles(), "el triangulo de la derecha no entra");
    assert!(!enc.escena.elementos.iter().any(|e| e.id == id_rect), "el rectangulo de la izquierda no entra");
    assert!(enc.escena.elementos.iter().any(|e| e.id == id_circulo), "con su id de siempre");
    let v = pintar(enc).unwrap();
    assert!(v.ancho > 300);
}

#[test]
fn sin_nada_que_ver_lo_dice() {
    let ctx = Contexto::default();
    assert!(encargo_de(&Escena::nueva(), &Mirada::default(), &ctx, |_| None).unwrap_err().contains("vacio"));
    let escena = escena_de(TRES_FIGURAS);
    assert!(encargo_de(&escena, &Mirada::de(Zona::Elegido), &ctx, |_| None).is_err());
}

#[test]
fn las_imagenes_de_la_zona_van_con_sus_pixeles() {
    let mut escena = Escena::nueva();
    let foto = pixpin_codec::ImagenRgba { ancho: 4, alto: 4, pixeles: [200, 30, 30, 255].repeat(16) };
    escena.anadir(crate::imagenes_lienzo::elemento_imagen(77, 0.0, 0.0, 200.0, 200.0));
    escena.anadir(crate::imagenes_lienzo::elemento_imagen(78, 5000.0, 5000.0, 20.0, 20.0));
    let enc = copiar(&escena, (-10.0, -10.0, 300.0, 300.0), (0.0, 0.0), true, |id| (id == 77).then(|| Pixeles::Rgba(foto.clone())));
    assert_eq!(enc.imagenes.len(), 1);
    assert_eq!(enc.escena.elementos.len(), 1);
    let v = pintar(enc).unwrap();
    let img = leer_png(&v.png);
    let c = pixel(&img, img.ancho / 2, img.alto / 2);
    assert!(c[0] > 150 && c[1] < 90, "la foto roja se ve: {c:?}");
}

/// **El bucle entero en la app**, con un modelo guionado que corrige: el
/// turno dibuja en el lienzo abierto, se mira (una imagen PNG de verdad
/// llega al modelo), mueve lo que dibujo, y todo queda en un solo Ctrl+Z.
#[test]
fn el_turno_dibuja_mira_corrige_y_se_deshace_de_una_vez() {
    use pixpin_ia::motor::{MotorGuionado, Respuesta};
    use crate::asistente::al_lienzo;
    use crate::asistente::turno::{Aviso, Turno};
    al_lienzo::vaciar_para_pruebas();
    let _editor = al_lienzo::EditorAbierto::nuevo(3);
    let mut escena = escena_de(TRES_FIGURAS);
    let antes = escena.cuantos_visibles();
    let siguiente = escena.siguiente_id;
    let motor = Arc::new(ComoNube(
        MotorGuionado::nuevo(vec![
            Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":0,"y":0,"texto":"Hola"}]}"#),
            Respuesta::de_texto("Escribi Hola."),
            Respuesta::de_llamada("transformar_elementos", &format!(r##"{{"que":"#{siguiente}","accion":"mover","dx":40}}"##)),
            Respuesta::de_texto("Movi el texto para que no tape nada."),
        ])
        .con_vision(),
    ));
    let historial = crate::asistente::turno::historial_para_el_modelo(&[], "escribe Hola en el lienzo", None);
    let mut t = Turno::lanzar_hacia(motor.clone(), "p".into(), historial, vec![], None, None);
    let mut texto = String::new();
    let hasta = Instant::now() + std::time::Duration::from_secs(60);
    while !t.acabado() && Instant::now() < hasta {
        al_lienzo::atender_en_editor(&mut escena, (400.0, 300.0));
        for a in t.recibir() {
            if let Aviso::Fin { resultado } = a {
                texto = resultado.unwrap().texto;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(t.acabado());
    assert!(texto.contains("Escribi Hola.") && texto.contains("Movi el texto"), "{texto}");
    assert_eq!(escena.cuantos_visibles(), antes + 1);
    // La tercera peticion lleva un PNG de verdad.
    let pedidas = motor.0.recibido.lock().unwrap();
    let vista = pedidas[2].mensajes.last().unwrap();
    let png = vista.partes.iter().find_map(|p| match p {
        pixpin_ia::motor::Parte::Imagen { bytes, .. } => Some(bytes.clone()),
        _ => None,
    });
    let img = leer_png(&png.expect("la revision lleva la imagen"));
    assert!(img.ancho.max(img.alto) <= pixpin_ia::ojos::LADO_MAXIMO);
    // Dibujar y corregir: un solo paso de deshacer.
    assert!(escena.deshacer());
    assert_eq!(escena.cuantos_visibles(), antes);
}

/// Sin vision, el mismo turno no mira ni corrige (y el guion sobrante no se
/// usa).
#[test]
fn sin_vision_el_turno_no_mira() {
    use pixpin_ia::motor::{MotorGuionado, Respuesta};
    use crate::asistente::al_lienzo;
    use crate::asistente::turno::Turno;
    al_lienzo::vaciar_para_pruebas();
    let _editor = al_lienzo::EditorAbierto::nuevo(4);
    let mut escena = escena_de(TRES_FIGURAS);
    let motor = Arc::new(MotorGuionado::nuevo(vec![
        Respuesta::de_llamada("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"texto","x":0,"y":0,"texto":"Hola"}]}"#),
        Respuesta::de_texto("Escribi Hola."),
    ]));
    let historial = crate::asistente::turno::historial_para_el_modelo(&[], "escribe Hola en el lienzo", None);
    let mut t = Turno::lanzar_hacia(motor.clone(), "p".into(), historial, vec![], None, None);
    let hasta = Instant::now() + std::time::Duration::from_secs(30);
    while !t.acabado() && Instant::now() < hasta {
        al_lienzo::atender_en_editor(&mut escena, (400.0, 300.0));
        let _ = t.recibir();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(t.acabado());
    let pedidas = motor.recibido.lock().unwrap();
    assert_eq!(pedidas.len(), 2);
    assert!(!pedidas.iter().any(|p| p.mensajes.iter().any(pixpin_ia::motor::Mensaje::tiene_imagen)));
}

/// Un motor guionado que se porta como la nube (la que revisa:
/// `turno::revisiones`).
struct ComoNube(pixpin_ia::motor::MotorGuionado);

impl pixpin_ia::Motor for ComoNube {
    fn completar(
        &self,
        p: &pixpin_ia::Peticion,
        al_trozo: &mut dyn FnMut(&pixpin_ia::motor::Trozo) -> pixpin_ia::motor::Seguir,
    ) -> Result<pixpin_ia::Respuesta, pixpin_ia::ErrorIa> {
        self.0.completar(p, al_trozo)
    }
    fn catalogo_completo(&self) -> bool {
        true
    }
    fn ve_imagenes(&self) -> bool {
        self.0.ve_imagenes()
    }
}
