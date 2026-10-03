//! Con la ventana entera pero **oculta** y una aplicacion de mentira
//! ([`Falsa`]) que dice que es cada renglon y apunta lo que se le pide: el
//! `RichEdit` de verdad, los incrustados pintados en memoria y el raton
//! con mensajes hechos a mano.

use std::cell::RefCell as Celda;
use std::collections::HashMap;
use std::rc::Rc as Comun;

use super::*;
use crate::editor::pruebas::{abrir_con, foto_de_prueba, pedido};
use crate::incrustados::{Archivo, Audio, Burbuja, Contenido, Medios, MensajeElegible, Transcribiendo};

const PDF: &str = "pixpin:files/guardados/pc/p1/notas/9-Plano.pdf";
const VOZ: &str = "pixpin:files/guardados/pc/p1/voz/voz-1.m4a";
const MSJ: &str = "pixpin:mensaje=PRJ0000001/MSG0000047";

/// Lo que la aplicacion de mentira sabe y lo que se le pidio.
#[derive(Default)]
struct Registro {
    fichas: HashMap<String, Ficha>,
    bloques: HashMap<String, String>,
    mensajes: Vec<MensajeElegible>,
    ordenes: Vec<OrdenAudio>,
    estado: Option<EstadoAudio>,
    pasar: Vec<String>,
    transcripcion: Option<Transcribiendo>,
}

struct Falsa(Comun<Celda<Registro>>);

impl Medios for Falsa {
    fn ficha(&mut self, ruta: &str) -> Option<Ficha> {
        self.0.borrow().fichas.get(ruta).cloned()
    }
    fn mensajes(&mut self) -> Vec<MensajeElegible> {
        self.0.borrow().mensajes.clone()
    }
    fn insertar_mensaje(&mut self, clave: &str) -> Option<String> {
        self.0.borrow().bloques.get(clave).cloned()
    }
    fn audio(&mut self, orden: OrdenAudio) -> Option<EstadoAudio> {
        let mut r = self.0.borrow_mut();
        if orden != OrdenAudio::Mirar {
            r.ordenes.push(orden);
        }
        r.estado.clone()
    }
    fn pasar_a_texto(&mut self, ruta: &str) -> Result<(), String> {
        self.0.borrow_mut().pasar.push(ruta.to_string());
        Ok(())
    }
    fn transcribiendo(&mut self) -> Option<Transcribiendo> {
        self.0.borrow().transcripcion.clone()
    }
}

fn plano() -> Ficha {
    Ficha::Archivo(Archivo {
        nombre: "Plano.pdf".into(),
        detalle: "1.2 MB · PDF".into(),
        falta: false,
    })
}

fn grua() -> Ficha {
    Ficha::Burbuja(Burbuja {
        hora: "10:42".into(),
        codigo: Some("#47·K7Q2".into()),
        contenido: Contenido::Texto("Pedir la grua para el jueves a las ocho".into()),
    })
}

fn nota_de_voz() -> Ficha {
    Ficha::Audio(Audio {
        nombre: "Nota de voz".into(),
        duracion_ms: 65_000,
        falta: false,
    })
}

fn registro() -> Comun<Celda<Registro>> {
    let r = Registro {
        fichas: HashMap::from([(PDF.to_string(), plano()), (MSJ.to_string(), grua()), (VOZ.to_string(), nota_de_voz())]),
        ..Default::default()
    };
    Comun::new(Celda::new(r))
}

fn abrir_falsa(texto: &str, r: &Comun<Celda<Registro>>, claro: bool) -> Estado {
    std::mem::forget(pixpin_shell::ComDelHilo::iniciar());
    let mut p = pedido(texto);
    p.integracion.medios = Some(Box::new(Falsa(r.clone())));
    p.rotulos.incrustados = crate::incrustados::RotulosIncrustados {
        pasar_a_texto: "Pasar a texto".into(),
        pasando: "Pasando a texto {pct} %".into(),
        borrado: "Mensaje borrado".into(),
        falta: "No está en este equipo".into(),
        ..Default::default()
    };
    let e = abrir_con(p, claro, (1000, 900));
    // Sitio de lo de encima (como al pintar la ventana).
    let _ = muestra(&e);
    e
}

fn cerrar(e: Estado) {
    desmontar(e);
}

fn en(e: &Estado, trozo: &str) -> usize {
    let t = leer(e.edit);
    t[..t.find(trozo).unwrap()].encode_utf16().count()
}

fn escondida(e: &Estado, p: usize) -> bool {
    let d = e.doc.as_ref().expect("el control da su documento");
    // SAFETY: rango del documento vivo del control.
    unsafe { d.Range(p as i32, p as i32 + 1).and_then(|r| r.GetFont()).and_then(|f| f.GetHidden()).unwrap_or(0) != 0 }
}

fn puestas() -> Vec<String> {
    imagenes::PUESTAS.with(|p| p.borrow().iter().map(|pu| pu.ruta.clone()).collect())
}

fn msg(e: &Estado, m: u32, x: i32, y: i32) -> MSG {
    MSG {
        hwnd: e.edit,
        message: m,
        wParam: WPARAM(0),
        lParam: LPARAM((((y as u16) as isize) << 16) | (x as u16) as isize),
        ..Default::default()
    }
}

/// Donde quedo pintado el incrustado de `ruta` y sus zonas.
fn sitio(ruta: &str) -> (RECT, Zonas) {
    let caja = imagenes::PUESTAS.with(|p| p.borrow().iter().find(|pu| pu.ruta == ruta).map(|pu| pu.caja.get())).unwrap();
    let zonas = MEMORIA.with(|m| m.borrow().pintados.get(ruta).map(|(_, _, z)| *z)).unwrap();
    (caja, zonas)
}

fn pulsar_en(e: &mut Estado, ruta: &str, zona: impl Fn(&Zonas) -> Option<pin::Caja>) -> bool {
    let (caja, zonas) = sitio(ruta);
    let z = zona(&zonas).expect("la zona esta");
    let m = msg(e, WM_LBUTTONDOWN, caja.left + z.x + z.an / 2, caja.top + z.y + z.al / 2);
    raton(e, &m)
}

#[test]
fn un_documento_de_fuera_se_copia_junto_a_la_nota_y_nace_pintado_como_tarjeta() {
    let r = registro();
    let copiados = Comun::new(Celda::new(Vec::<String>::new()));
    let c = copiados.clone();
    std::mem::forget(pixpin_shell::ComDelHilo::iniciar());
    let mut p = pedido("Obra");
    p.integracion.medios = Some(Box::new(Falsa(r.clone())));
    p.adjuntar = Box::new(move |o: &Path| {
        c.borrow_mut().push(nombre_de(o));
        Some(PDF.to_string())
    });
    let mut e = abrir_con(p, false, (1000, 760));
    let pdf = foto_de_prueba("x.png", 2, 2).with_file_name("Plano.pdf");
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    assert_eq!(meter_documentos(&mut e, std::slice::from_ref(&pdf)), 1);
    // Copiado (lo hace la aplicacion) y en su renglon, el del movil.
    assert_eq!(*copiados.borrow(), vec!["Plano.pdf".to_string()]);
    assert_eq!(markdown(&e), format!("Obra\n![Plano.pdf]({PDF})"));
    // Nace pintado: su tarjeta encima y su Markdown escondido.
    assert_eq!(puestas(), vec![PDF.to_string()]);
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    assert!(escondida(&e, en(&e, "![")));
    assert!(escondida(&e, en(&e, "Plano.pdf")));
    // Caso negativo: un fichero que no esta no entra.
    assert_eq!(meter_documentos(&mut e, &[PathBuf::from("C:\\no\\esta.pdf")]), 0);
    cerrar(e);
}

#[test]
fn un_mensaje_del_chat_se_enlaza_sin_copiar_y_se_pinta_como_su_burbuja() {
    let r = registro();
    r.borrow_mut().bloques.insert("k47".into(), format!("[Pedir la grua]({MSJ})"));
    let mut e = abrir_falsa("Pendientes", &r, false);
    assert!(meter_mensaje(&mut e, "k47"));
    assert_eq!(markdown(&e), format!("Pendientes\n[Pedir la grua]({MSJ})"));
    let _ = muestra(&e);
    assert!(puestas().contains(&MSJ.to_string()));
    // La burbuja: un clic en ella abre el mensaje en el chat (lo decide la
    // aplicacion por su direccion).
    let abiertos = Comun::new(Celda::new(Vec::<String>::new()));
    let a = abiertos.clone();
    e.integracion.abrir = Some(Box::new(move |ruta: &str| a.borrow_mut().push(ruta.to_string())));
    assert!(pulsar_en(&mut e, MSJ, |z| Some(z.tarjeta)));
    assert_eq!(*abiertos.borrow(), vec![MSJ.to_string()]);
    // Caso negativo: un mensaje que la aplicacion no da no mete nada.
    assert!(!meter_mensaje(&mut e, "no-esta"));
    cerrar(e);
}

#[test]
fn la_burbuja_lleva_su_hora_y_su_chapa_como_en_el_chat() {
    let col = pin::Colores::del_chat(false, 0xfdfcf9);
    let r = crate::incrustados::RotulosIncrustados::default();
    let corta = |codigo: Option<&str>| Ficha::Burbuja(Burbuja {
        hora: "10:42".into(),
        codigo: codigo.map(str::to_string),
        contenido: Contenido::Texto("Ok".into()),
    });
    let con = pin::pintar(&corta(Some("#47·K7Q2")), &ComoSuena::default(), 720, &col, &r, 1.0).unwrap();
    let sin = pin::pintar(&corta(None), &ComoSuena::default(), 720, &col, &r, 1.0).unwrap();
    // Con chapa la burbuja es mas ancha: el pie manda sobre un «Ok».
    assert!(con.zonas.tarjeta.an > sin.zonas.tarjeta.an + 30, "{} {}", con.zonas.tarjeta.an, sin.zonas.tarjeta.an);
    // Y lleva el fondo de la chapa (el filete del chat) en su pie.
    let px = leer_mapa(&con);
    assert!(px.contains(&col.filete), "sin chapa");
    assert!(px.contains(&col.burbuja), "sin burbuja");
    for p in [con, sin] {
        // SAFETY: mapas de esta prueba.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(p.mapa.0));
        }
    }
}

/// Los pixeles de un mapa pintado, en `0xRRGGBB`.
fn leer_mapa(p: &pin::Pintado) -> Vec<u32> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: p.an,
            biHeight: -p.al,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut v = vec![0u8; (p.an * p.al * 4) as usize];
    // SAFETY: DC de pantalla prestado; el bufer mide lo que se pide.
    unsafe {
        let dc = GetDC(None);
        GetDIBits(dc, p.mapa, 0, p.al as u32, Some(v.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        ReleaseDC(None, dc);
    }
    v.chunks_exact(4).map(|c| (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32).collect()
}

#[test]
fn un_audio_con_transcripcion_salta_al_minuto_de_su_marca_y_resalta_lo_que_suena() {
    let r = registro();
    let md = format!("![Nota de voz]({VOZ})\n\n[0:00] llamar al aparejador\n\n[0:21] y pedir el presupuesto");
    let mut e = abrir_falsa(&md, &r, false);
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    // La marca: los corchetes escondidos, el minuto a la vista.
    let p = en(&e, "[0:21]");
    assert!(escondida(&e, p));
    assert!(!escondida(&e, p + 1));
    assert!(escondida(&e, p + 5));
    // Con transcripcion no hay pastilla de pasar a texto.
    assert!(sitio(VOZ).1.texto.is_none());
    // Un clic en el minuto salta el audio de encima a ese punto.
    let texto = leer(e.edit);
    assert_eq!(marca_en(&texto, p + 2), Some((VOZ.to_string(), 21_000)));
    let mut pt = POINT::default();
    enviar(e.edit, EM_POSFROMCHAR, &mut pt as *mut _ as usize, (p + 2) as isize);
    let m = msg(&e, WM_LBUTTONDOWN, pt.x + 2, pt.y + 4);
    assert!(raton(&mut e, &m));
    assert_eq!(r.borrow().ordenes, vec![OrdenAudio::Saltar(VOZ.into(), 21_000)]);
    // Suena por el segundo 23: se resalta el segundo parrafo.
    r.borrow_mut().estado = Some(EstadoAudio {
        ruta: VOZ.into(),
        sonando: true,
        posicion_ms: 23_000,
        duracion_ms: 65_000,
        velocidad: 1.0,
    });
    latido(&mut e);
    let segundo = md_vivo::linea_de(&md_vivo::lineas(&leer(e.edit)), p);
    assert_eq!(MEMORIA.with(|m| m.borrow().resaltado), Some(segundo));
    // Caso negativo: un clic en el texto del parrafo no salta.
    assert_eq!(marca_en(&texto, en(&e, "presupuesto")), None);
    cerrar(e);
}

#[test]
fn el_reproductor_toca_cambia_de_velocidad_y_va_al_punto_de_la_barra() {
    let r = registro();
    let mut e = abrir_falsa(&format!("![Nota de voz]({VOZ})\n\n[0:00] hola"), &r, true);
    assert!(pulsar_en(&mut e, VOZ, |z| z.tocar));
    assert!(pulsar_en(&mut e, VOZ, |z| z.velocidad));
    let o = r.borrow().ordenes.clone();
    assert_eq!(o, vec![OrdenAudio::Alternar(VOZ.into()), OrdenAudio::Velocidad]);
    // La barra, con el audio ya cargado: va al punto (la mitad).
    r.borrow_mut().estado = Some(EstadoAudio {
        ruta: VOZ.into(),
        duracion_ms: 65_000,
        velocidad: 1.0,
        ..Default::default()
    });
    latido(&mut e);
    assert!(pulsar_en(&mut e, VOZ, |z| z.barra));
    match r.borrow().ordenes.last() {
        Some(OrdenAudio::IrA(ruta, f)) => assert!(ruta == VOZ && (*f - 0.5).abs() < 0.05, "{f}"),
        otra => panic!("{otra:?}"),
    }
    cerrar(e);
}

#[test]
fn un_audio_sin_transcripcion_ofrece_pasar_a_texto_y_su_texto_entra_debajo() {
    let r = registro();
    let mut e = abrir_falsa(&format!("Visita\n![Nota de voz]({VOZ})\nfin"), &r, false);
    assert!(sitio(VOZ).1.texto.is_some(), "sin transcripcion, la pastilla");
    assert!(pulsar_en(&mut e, VOZ, |z| z.texto));
    assert_eq!(r.borrow().pasar, vec![VOZ.to_string()]);
    // Acaba: el texto entra debajo del audio, con sus marcas.
    r.borrow_mut().transcripcion = Some(Transcribiendo {
        ruta: VOZ.into(),
        avance: 1.0,
        hecho: Some(Ok("[0:00] hola\n\n[0:05] adios".into())),
    });
    latido(&mut e);
    assert_eq!(markdown(&e), format!("Visita\n![Nota de voz]({VOZ})\n\n[0:00] hola\n\n[0:05] adios\nfin"));
    // Caso negativo: si ya la lleva, no se pone otra vez.
    assert!(!poner_letra(&mut e, VOZ, "[0:00] otra"));
    cerrar(e);
}

#[test]
fn el_markdown_va_y_vuelve_igual_con_todos_los_incrustados() {
    let r = registro();
    let md = format!(
        "# Obra\n![Plano.pdf]({PDF})\n[Pedir la grua]({MSJ})\n![Nota de voz]({VOZ})\n\n[0:00] hola\n\n[Planta](pixpin:hoja=PRJ0000001/HOJA000001)\nfin"
    );
    let mut e = abrir_falsa(&md, &r, false);
    pintar(&mut e, None);
    assert_eq!(markdown(&e), md);
    // La hoja no la conoce la aplicacion de mentira: se queda como texto.
    assert_eq!(puestas(), vec![PDF.to_string(), MSJ.to_string(), VOZ.to_string()]);
    cerrar(e);
}

#[test]
fn un_mensaje_borrado_y_un_fichero_ausente_se_dicen_sin_ofrecer_lo_que_no_hay() {
    let r = registro();
    {
        let mut x = r.borrow_mut();
        x.fichas.insert(MSJ.into(), Ficha::Borrado("Pedir la grua".into()));
        x.fichas.insert(
            VOZ.into(),
            Ficha::Audio(Audio {
                nombre: "Nota de voz".into(),
                duracion_ms: 0,
                falta: true,
            }),
        );
    }
    let e = abrir_falsa(&format!("[Pedir la grua]({MSJ})\n![Nota de voz]({VOZ})"), &r, false);
    // Los dos se pintan (el renglon no se queda a la vista)...
    assert_eq!(puestas(), vec![MSJ.to_string(), VOZ.to_string()]);
    // ...pero el audio que falta no se toca ni se pasa a texto.
    let z = sitio(VOZ).1;
    assert!(z.tocar.is_none() && z.barra.is_none() && z.texto.is_none() && z.velocidad.is_none());
    cerrar(e);
}

#[test]
fn sin_aplicacion_o_sin_ficha_los_renglones_se_quedan_como_texto() {
    // Sin aplicacion: nada encima y el texto a la vista.
    let mut e = abrir_con(pedido(&format!("![Plano.pdf]({PDF})")), false, (1000, 760));
    let _ = muestra(&e);
    pintar(&mut e, None);
    assert!(puestas().is_empty());
    desmontar(e);
    // Con aplicacion que no sabe que es: tampoco.
    let r = Comun::new(Celda::new(Registro::default()));
    let e = abrir_falsa(&format!("![Plano.pdf]({PDF})\nfin"), &r, false);
    assert!(puestas().is_empty());
    cerrar(e);
}

#[test]
fn soltar_un_pdf_lo_mete_como_tarjeta_y_una_foto_como_foto() {
    let r = registro();
    std::mem::forget(pixpin_shell::ComDelHilo::iniciar());
    let mut p = pedido("antes");
    p.integracion.medios = Some(Box::new(Falsa(r.clone())));
    p.adjuntar = Box::new(|o: &Path| Some(format!("pixpin:files/guardados/pc/p1/notas/9-{}", nombre_de(o))));
    let mut e = abrir_con(p, false, (1000, 760));
    let foto = foto_de_prueba("obra.png", 40, 30);
    let pdf = foto.with_file_name("Plano.pdf");
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    assert_eq!(fotos::soltar(&mut e, vec![foto, pdf], None), 2);
    let md = markdown(&e);
    assert!(md.contains("![obra](pixpin:files/guardados/pc/p1/notas/9-obra.png)"), "{md}");
    assert!(md.contains(&format!("![Plano.pdf]({PDF})")), "{md}");
    cerrar(e);
}

// ---------------------------------------------------------------------------
// Muestras: `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas
// muestra_de_incrustados -- --ignored`

fn muestra_de(claro: bool, nombre: &str) {
    let r = registro();
    let hoja = foto_de_prueba("planta-hoja.png", 900, 600);
    let obra = foto_de_prueba("obra-chat.png", 1200, 800);
    {
        let mut x = r.borrow_mut();
        x.fichas.insert(
            "pixpin:hoja=PRJ0000001/HOJA000001".into(),
            Ficha::Burbuja(Burbuja {
                hora: "ayer 18:05".into(),
                codigo: Some("#31·K7Q2".into()),
                contenido: Contenido::Hoja {
                    nombre: "Planta baja".into(),
                    miniatura: Some(hoja),
                    clase: "2D".into(),
                },
            }),
        );
        x.fichas.insert(
            "pixpin:mensaje=PRJ0000001/MSG0000052".into(),
            Ficha::Burbuja(Burbuja {
                hora: "11:03".into(),
                codigo: Some("#52·K7Q2".into()),
                contenido: Contenido::Foto {
                    ruta: obra,
                    pie: "La zanja ya esta abierta".into(),
                },
            }),
        );
        x.estado = Some(EstadoAudio {
            ruta: VOZ.into(),
            sonando: true,
            posicion_ms: 23_000,
            duracion_ms: 65_000,
            velocidad: 1.5,
        });
    }
    let md = format!(
        "# Visita de obra\nLo que se hablo el jueves, con el plano y la nota de voz del aparejador.\n\
![Plano.pdf]({PDF})\n\
[Pedir la grua para el jueves a las ocho]({MSJ})\n\
![Nota de voz]({VOZ})\n\n\
[0:00] Llamar al aparejador antes de las nueve para cerrar lo de la grua.\n\n\
[0:21] Y pedir el presupuesto de la zanja, que el de la semana pasada no incluia el relleno.\n\n\
[0:48] Lo demas, el lunes en la obra.\n\
[Planta baja](pixpin:hoja=PRJ0000001/HOJA000001)\n\
[La zanja](pixpin:mensaje=PRJ0000001/MSG0000052)\n\
Fin de la visita."
    );
    let mut e = abrir_falsa(&md, &r, claro);
    latido(&mut e);
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    let img = muestra(&e);
    crate::editor::pruebas::guardar_png(&img, nombre);
    // Y la parte de abajo (la hoja y la foto del chat).
    let abajo = POINT { x: 0, y: 700 };
    enviar(e.edit, EM_SETSCROLLPOS, 0, &abajo as *const _ as isize);
    let img = muestra(&e);
    crate::editor::pruebas::guardar_png(&img, &nombre.replace(".png", "-abajo.png"));
    cerrar(e);
}

#[test]
#[ignore = "genera PNG para mirarlos"]
fn muestra_de_incrustados() {
    muestra_de(false, "nota-md-incrustados.png");
    muestra_de(true, "nota-md-incrustados-clara.png");
}
