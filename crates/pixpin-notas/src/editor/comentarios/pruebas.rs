//! Pruebas de los comentarios con la ventana de verdad pero **oculta**.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::super::{
    Opciones, Pedido, Rotulos, aplicar_cambio, desmontar, elegir, leer, markdown, montar, muestra,
};
use super::*;
use crate::panel_comentarios::RotulosComentarios;

const NOTA: &str =
    "# Obra\nLa losa del segundo piso ya esta hormigonada.\nFalta el curado de la losa.\n";

/// El «disco» de la prueba: lo que hay en el fichero y cuantas veces se
/// escribio.
#[derive(Clone, Default)]
struct Disco {
    texto: Rc<RefCell<Option<String>>>,
    escrituras: Rc<RefCell<usize>>,
}

impl Disco {
    fn con(t: &str) -> Disco {
        let d = Disco::default();
        *d.texto.borrow_mut() = Some(t.to_string());
        d
    }
    fn leido(&self) -> md::Comentarios {
        md::leer(self.texto.borrow().as_deref().unwrap_or("")).unwrap()
    }
}

fn rotulos() -> Rotulos {
    Rotulos {
        meses: "ene feb mar abr may jun jul ago sept oct nov dic".into(),
        compartir: "Compartir".into(),
        nueva: "Nota nueva".into(),
        comentarios: RotulosComentarios {
            comentarios: "Comentarios".into(),
            comentar: "Comentar".into(),
            sin_comentarios: "No hay comentarios".into(),
            pista: "Elige un trozo del texto y pulsa Comentar (Ctrl+Alt+M)".into(),
            sin_ancla: "Su texto ya no está en la nota".into(),
            responder_pista: "Responder…".into(),
            responder: "Responder".into(),
            guardar: "Guardar".into(),
            cancelar: "Cancelar".into(),
            editar: "Editar".into(),
            borrar: "Borrar".into(),
            resuelto: "resuelto".into(),
            ver_resueltos: "Ver resueltos".into(),
            ocultar_resueltos: "Ocultar resueltos".into(),
            editado: "editado".into(),
            borrar_hilo: "¿Borrar?".into(),
        },
        ..Default::default()
    }
}

fn de(d: &Disco, autor: &str) -> DeComentarios {
    let (l, g) = (d.clone(), d.clone());
    DeComentarios {
        autor: autor.into(),
        aparato: "K7Q2".into(),
        leer: Some(Box::new(move || {
            Some(l.texto.borrow().clone().unwrap_or_default())
        })),
        guardar: Some(Box::new(move |t: &str| {
            *g.texto.borrow_mut() = Some(t.to_string());
            *g.escrituras.borrow_mut() += 1;
            true
        })),
    }
}

fn abrir_con(texto: &str, d: DeComentarios, claro: bool, tamano: (i32, i32)) -> Estado {
    montar(
        Pedido {
            texto: texto.into(),
            rotulos: rotulos(),
            nombre_de_fichero: None,
            colocacion: None,
            resolver: Box::new(|r: &str| Some(PathBuf::from(r))),
            adjuntar: Box::new(|_: &Path| None),
            compartir: None,
            integracion: Default::default(),
            comentarios: d,
        },
        Opciones {
            oculto: true,
            tamano: Some(tamano),
            claro: Some(claro),
        },
    )
    .expect("la ventana oculta se monta")
}

fn abrir(texto: &str, d: &Disco) -> Estado {
    abrir_con(texto, de(d, "Portátil"), false, (1100, 800))
}

/// Elige la aparicion `n` de `que` en el texto del control.
fn elegir_texto(e: &Estado, que: &str, n: usize) -> (usize, usize) {
    let t = leer(e.edit);
    let i = t.match_indices(que).nth(n).unwrap().0;
    let a = t[..i].encode_utf16().count();
    let b = a + que.encode_utf16().count();
    elegir(e.edit, a, b);
    (a, b)
}

fn escribir_en_el_cuadro(e: &Estado, t: &str) {
    let h = e.comentarios.compositor.expect("hay cuadro de escribir");
    // SAFETY: control de la prueba.
    unsafe {
        let _ = SetWindowTextW(h, &HSTRING::from(t));
    }
}

/// Comenta lo elegido con `texto`, como Ctrl+Alt+M, escribir e Intro.
fn comentar_con(e: &mut Estado, texto: &str) {
    comentar(e);
    escribir_en_el_cuadro(e, texto);
    enviar_borrador(e);
}

fn tarjeta(id: &str) -> Option<crate::disposicion::Caja> {
    panel::VISTA.with(|v| {
        v.borrow()
            .as_ref()?
            .tarjetas
            .iter()
            .find(|(x, _)| x == id)
            .map(|(_, c)| *c)
    })
}

#[test]
fn comentar_lo_elegido_abre_el_panel_resalta_y_guarda_fuera_del_markdown() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    assert!(
        !panel::ABIERTO.with(|a| a.get()),
        "sin comentarios, el panel nace cerrado"
    );
    let (a, b) = elegir_texto(&e, "segundo piso", 0);
    comentar(&mut e);
    assert!(panel::ABIERTO.with(|x| x.get()), "comentar abre el panel");
    assert!(matches!(
        e.comentarios.borrador,
        Some(Borrador::Nuevo { .. })
    ));
    assert!(e.comentarios.compositor.is_some());
    escribir_en_el_cuadro(&e, "¿Seguro que es el segundo?");
    enviar_borrador(&mut e);
    assert!(e.comentarios.compositor.is_none() && e.comentarios.borrador.is_none());
    let h = &e.comentarios.datos.comentarios[0];
    assert_eq!(
        (h.ancla.cita.as_str(), h.texto.as_str(), h.autor.as_str()),
        ("segundo piso", "¿Seguro que es el segundo?", "Portátil")
    );
    assert_eq!(e.comentarios.rangos, [Some((a, b))]);
    assert_eq!(e.comentarios.activo.as_deref(), Some(h.id.as_str()));
    assert_eq!(panel::CONTADOR.with(|c| c.get()), 1);
    // Su tarjeta esta en el panel, a la altura de su frase.
    assert!(tarjeta(&h.id).is_some());
    // Guardado aparte, y el Markdown no lleva nada de esto.
    assert_eq!(*d.escrituras.borrow(), 1);
    assert_eq!(d.leido().comentarios[0].texto, "¿Seguro que es el segundo?");
    assert_eq!(markdown(&e), NOTA);
    // El texto comentado tiene fondo; el de al lado no.
    let fondo_en = |p: usize| -> i32 {
        // SAFETY: documento de la prueba.
        unsafe {
            e.doc
                .as_ref()
                .unwrap()
                .Range(p as i32, p as i32 + 1)
                .unwrap()
                .GetFont()
                .unwrap()
                .GetBackColor()
                .unwrap()
        }
    };
    let co = panel::colores(&e.estilos.tema);
    assert_eq!(fondo_en(a + 1), bgr(co.resaltado_activo) as i32);
    assert_ne!(fondo_en(a - 3), bgr(co.resaltado_activo) as i32);
    desmontar(e);
}

#[test]
fn sin_nada_elegido_se_comenta_la_palabra_y_en_un_blanco_nada() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    let (a, _) = elegir_texto(&e, "curado", 0);
    elegir(e.edit, a + 2, a + 2);
    comentar_con(&mut e, "¿Cuantos dias?");
    assert_eq!(e.comentarios.datos.comentarios[0].ancla.cita, "curado");
    // Caso negativo: el cursor entre dos blancos no comenta nada.
    let mut e2 = abrir("a   b", &Disco::default());
    elegir(e2.edit, 2, 2);
    comentar(&mut e2);
    assert!(e2.comentarios.borrador.is_none() && e2.comentarios.compositor.is_none());
    desmontar(e2);
    desmontar(e);
}

#[test]
fn un_comentario_vacio_no_se_envia_y_cancelar_no_deja_rastro() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "losa", 1);
    comentar(&mut e);
    escribir_en_el_cuadro(&e, "   ");
    enviar_borrador(&mut e);
    assert!(
        e.comentarios.datos.comentarios.is_empty(),
        "vacio no se crea"
    );
    assert!(e.comentarios.compositor.is_some(), "y se sigue escribiendo");
    cancelar(&mut e);
    assert!(e.comentarios.compositor.is_none() && e.comentarios.borrador.is_none());
    assert_eq!(*d.escrituras.borrow(), 0, "nada que guardar");
    desmontar(e);
}

#[test]
fn intro_envia_y_esc_cancela_en_el_cuadro() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "losa", 0);
    comentar(&mut e);
    escribir_en_el_cuadro(&e, "con Intro");
    let h = e.comentarios.compositor.unwrap();
    let intro = MSG {
        hwnd: h,
        message: WM_KEYDOWN,
        wParam: WPARAM(super::super::VK_RETURN.0 as usize),
        ..Default::default()
    };
    assert!(tecla(&mut e, &intro));
    assert_eq!(e.comentarios.datos.comentarios.len(), 1);
    elegir_texto(&e, "curado", 0);
    comentar(&mut e);
    let esc = MSG {
        hwnd: e.comentarios.compositor.unwrap(),
        message: WM_KEYDOWN,
        wParam: WPARAM(super::super::VK_ESCAPE.0 as usize),
        ..Default::default()
    };
    assert!(tecla(&mut e, &esc));
    assert_eq!(e.comentarios.datos.comentarios.len(), 1);
    // Una tecla de la nota no es del cuadro.
    let otra = MSG {
        hwnd: e.edit,
        ..intro
    };
    assert!(!tecla(&mut e, &otra));
    desmontar(e);
}

#[test]
fn responder_editar_resolver_y_ver_los_resueltos() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "curado", 0);
    comentar_con(&mut e, "¿Cuantos dias?");
    let id = e.comentarios.datos.comentarios[0].id.clone();
    hacer(&mut e, Accion::Responder(id.clone()));
    escribir_en_el_cuadro(&e, "Siete");
    hacer(&mut e, Accion::Enviar);
    let r = e.comentarios.datos.comentarios[0].respuestas[0].id.clone();
    editar(&mut e, &r);
    escribir_en_el_cuadro(&e, "Siete, regando");
    hacer(&mut e, Accion::Enviar);
    assert_eq!(
        e.comentarios.datos.comentarios[0].respuestas[0].texto,
        "Siete, regando"
    );
    assert!(
        e.comentarios.datos.comentarios[0].respuestas[0]
            .editado
            .is_some()
    );

    hacer(&mut e, Accion::Resolver(id.clone()));
    assert!(e.comentarios.datos.comentarios[0].resuelto);
    assert_eq!(panel::CONTADOR.with(|c| c.get()), 0);
    assert!(tarjeta(&id).is_none(), "resuelto, fuera del panel");
    hacer(&mut e, Accion::Filtro);
    assert!(tarjeta(&id).is_some(), "con el filtro se ve");
    hacer(&mut e, Accion::Resolver(id.clone()));
    assert!(!e.comentarios.datos.comentarios[0].resuelto, "y se reabre");
    assert!(d.leido().comentarios[0].respuestas.len() == 1);
    // Borrar la respuesta y luego el hilo.
    borrar(&mut e, &r);
    assert!(e.comentarios.datos.comentarios[0].respuestas.is_empty());
    borrar(&mut e, &id);
    assert!(e.comentarios.datos.comentarios.is_empty());
    assert!(
        d.leido().comentarios.is_empty(),
        "borrado tambien en disco (no vuelve al sincronizar)"
    );
    desmontar(e);
}

#[test]
fn el_comentario_sigue_a_su_texto_al_escribir_y_se_guarda_al_dia() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "ya esta hormigonada", 0);
    comentar_con(&mut e, "¿Seguro?");
    // Se escribe delante y dentro de la cita.
    let viejo = leer(e.edit);
    let nuevo = viejo
        .replace("# Obra\n", "# Obra en Lima\nIntro nueva.\n")
        .replace("ya esta hormigonada", "ya esta casi hormigonada");
    aplicar_cambio(e.edit, &viejo, &nuevo);
    pintar(&mut e, None);
    let (a, b) = e.comentarios.rangos[0].expect("sigue anclado");
    let u: Vec<u16> = leer(e.edit).encode_utf16().collect();
    assert_eq!(
        String::from_utf16_lossy(&u[a..b]),
        "ya esta casi hormigonada"
    );
    // Al guardar, el ancla va al dia en el fichero.
    let md = markdown(&e);
    guardar(&mut e, &md);
    let h = &d.leido().comentarios[0];
    assert_eq!(h.ancla.cita, "ya esta casi hormigonada");
    assert_eq!(
        h.ancla.pos,
        md[..md.find("ya esta casi").unwrap()]
            .encode_utf16()
            .count()
    );
    desmontar(e);
}

#[test]
fn si_su_texto_se_borra_el_comentario_queda_arriba_sin_ancla_y_no_se_pierde() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "del segundo piso", 0);
    comentar_con(&mut e, "¿Que piso?");
    let id = e.comentarios.datos.comentarios[0].id.clone();
    let viejo = leer(e.edit);
    aplicar_cambio(e.edit, &viejo, &viejo.replace("del segundo piso ", ""));
    pintar(&mut e, None);
    assert_eq!(e.comentarios.rangos, [None]);
    let c = tarjeta(&id).expect("sigue en el panel");
    let zona = panel::VISTA.with(|v| v.borrow().as_ref().unwrap().zona_movil);
    assert!(c.abajo() <= zona.y, "arriba, en la parte de los sin ancla");
    let md = markdown(&e);
    guardar(&mut e, &md);
    assert_eq!(
        d.leido().comentarios[0].ancla.cita,
        "del segundo piso",
        "con su cita de siempre"
    );
    desmontar(e);
}

#[test]
fn con_comentarios_abiertos_la_nota_abre_el_panel_y_los_resalta() {
    let mut previo = md::Comentarios::default();
    let t: Vec<u16> = NOTA.encode_utf16().collect();
    let i = NOTA.find("curado").unwrap();
    let a = NOTA[..i].encode_utf16().count();
    let quien = Quien {
        autor: "Teléfono".into(),
        aparato: "MOVI".into(),
    };
    previo
        .nuevo(md::ancla_de(&t, a, a + 6).unwrap(), &quien, 1, "del movil")
        .unwrap();
    let d = Disco::con(&md::escribir(&previo));
    let e = abrir(NOTA, &d);
    // Las tarjetas no salen solas (1-oct): solo con el boton, que dice
    // cuantos hay; lo comentado si se ve resaltado.
    assert!(!panel::ABIERTO.with(|x| x.get()));
    assert!(panel::VISTA.with(|v| v.borrow().is_none()));
    assert_eq!(e.comentarios.rangos, [Some((a, a + 6))]);
    assert_eq!(panel::CONTADOR.with(|c| c.get()), 1);
    assert_eq!(*d.escrituras.borrow(), 0, "abrir no escribe nada");
    desmontar(e);
}

/// Una nota con dos comentarios ya guardados: en `curado` y en `segundo`.
fn con_dos(texto: &str, que: [&str; 2], tamano: (i32, i32)) -> (Estado, [String; 2]) {
    let t: Vec<u16> = texto.encode_utf16().collect();
    let quien = Quien {
        autor: "Teléfono".into(),
        aparato: "MOVI".into(),
    };
    let mut c = md::Comentarios::default();
    let ids = que.map(|q| {
        let a = texto[..texto.find(q).unwrap()].encode_utf16().count();
        c.nuevo(
            md::ancla_de(&t, a, a + q.encode_utf16().count()).unwrap(),
            &quien,
            1,
            &format!("sobre {q}"),
        )
        .unwrap()
    });
    let d = Disco::con(&md::escribir(&c));
    (abrir_con(texto, de(&d, "Portátil"), false, tamano), ids)
}

fn ancho_del_papel() -> i32 {
    super::super::VISTA.with(|v| v.borrow().as_ref().unwrap().disp.cuerpo.an)
}

#[test]
fn las_tarjetas_solo_salen_con_el_boton_y_cada_una_a_la_altura_de_su_frase() {
    let (mut e, [a, b]) = con_dos(NOTA, ["segundo piso", "curado"], (1100, 800));
    let entero = ancho_del_papel();
    assert!(!panel::ABIERTO.with(|x| x.get()), "sin el boton no se ven");
    assert!(tarjeta(&a).is_none());
    // El boton de la cabecera las ensena (conmutador).
    super::super::clic(&mut e, crate::disposicion::Boton::Comentarios, &mut |_| {
        true
    });
    assert!(panel::ABIERTO.with(|x| x.get()));
    assert_eq!(
        ancho_del_papel(),
        entero,
        "con sitio, van encima del margen del papel, sin quitarle nada"
    );
    let (ta, tb) = (tarjeta(&a).unwrap(), tarjeta(&b).unwrap());
    let rango = |i: usize| e.comentarios.rangos[i].unwrap().0;
    assert_eq!(ta.y, alto_de(&e, rango(0)), "a la altura de su frase");
    assert!(
        tb.y >= alto_de(&e, rango(1)) && ta.abajo() <= tb.y,
        "la de debajo baja, sin solaparse"
    );
    // En el margen derecho del papel, a la derecha de lo escrito.
    let caja = panel::CAJA.with(|c| c.get()).unwrap();
    assert!(
        ta.x >= caja.x - crate::panel_comentarios::ADELANTE_PX * 2
            && tb.x >= caja.x - crate::panel_comentarios::ADELANTE_PX * 2
    );
    // Otra vez el boton: se van, y el papel vuelve a ser entero.
    super::super::clic(&mut e, crate::disposicion::Boton::Comentarios, &mut |_| {
        true
    });
    assert!(!panel::ABIERTO.with(|x| x.get()));
    assert_eq!(
        ancho_del_papel(),
        entero,
        "sin tarjetas no se reserva margen"
    );
    desmontar(e);
}

#[test]
fn un_clic_en_una_tarjeta_desliza_la_nota_hasta_su_texto_y_lo_resalta() {
    let mut nota = String::from("# Larga\n");
    for i in 0..45 {
        nota.push_str(&format!(
            "Renglon {i} de relleno para que la nota no quepa.\n"
        ));
    }
    nota.push_str("Aqui va la losa del final.\n");
    for i in 45..90 {
        nota.push_str(&format!("Renglon {i} de relleno debajo.\n"));
    }
    let (mut e, [a, _]) = con_dos(&nota, ["losa del final", "Renglon 3 "], (1100, 700));
    alternar_panel(&mut e);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    componer(&mut e);
    let (p, _) = e.comentarios.rangos[0].unwrap();
    let y_del_texto = |e: &Estado| {
        let mut q = POINT::default();
        enviar(
            e.edit,
            EM_POSFROMCHAR,
            &mut q as *mut _ as usize,
            p as isize,
        );
        q.y
    };
    let (visible, _) = crate::imagenes::medidas(e.edit);
    assert!(y_del_texto(&e) > visible, "al principio su texto no se ve");
    hacer(&mut e, Accion::Tarjeta(a.clone()));
    let y = y_del_texto(&e);
    assert!(
        y > 0 && y < visible * 3 / 4,
        "la nota fue hasta su texto: {y} de {visible}"
    );
    assert_eq!(seleccion(e.edit), (p, p));
    assert_eq!(e.comentarios.activo.as_deref(), Some(a.as_str()));
    // Resaltado como el elegido, y su tarjeta sigue al texto.
    let co = panel::colores(&e.estilos.tema);
    // SAFETY: documento de la prueba.
    let fondo = unsafe {
        e.doc
            .as_ref()
            .unwrap()
            .Range(p as i32, p as i32 + 1)
            .unwrap()
            .GetFont()
            .unwrap()
            .GetBackColor()
            .unwrap()
    };
    assert_eq!(fondo, bgr(co.resaltado_activo) as i32);
    assert_eq!(tarjeta(&a).unwrap().y, alto_de(&e, p));
    desmontar(e);
}

#[test]
fn un_clic_en_un_texto_comentado_ensena_las_tarjetas_y_elige_la_suya() {
    let (mut e, [_, b]) = con_dos(NOTA, ["segundo piso", "curado"], (1100, 800));
    let (p, _) = e.comentarios.rangos[1].unwrap();
    elegir(e.edit, p + 2, p + 2);
    let suelta = MSG {
        hwnd: e.edit,
        message: WM_LBUTTONUP,
        ..Default::default()
    };
    seguir(&mut e, &suelta);
    assert!(panel::ABIERTO.with(|x| x.get()));
    assert_eq!(e.comentarios.activo.as_deref(), Some(b.as_str()));
    assert!(tarjeta(&b).is_some());
    // Caso negativo: un clic fuera de lo comentado no las abre.
    alternar_panel(&mut e);
    elegir(e.edit, 1, 1);
    seguir(&mut e, &suelta);
    assert!(!panel::ABIERTO.with(|x| x.get()));
    desmontar(e);
}

#[test]
fn con_la_ventana_estrecha_las_tarjetas_van_en_un_cajon_encima_del_texto() {
    let (mut e, [a, _]) = con_dos(NOTA, ["segundo piso", "curado"], (640, 800));
    let entero = ancho_del_papel();
    alternar_panel(&mut e);
    assert!(panel::CAJON.with(|c| c.get()));
    assert_eq!(ancho_del_papel(), entero, "el papel no pierde nada");
    let caja = panel::CAJA.with(|c| c.get()).unwrap();
    let papel = super::super::VISTA.with(|v| v.borrow().as_ref().unwrap().disp.cuerpo);
    assert!(
        caja.x < papel.derecha() && caja.x > papel.x,
        "encima del texto, por la derecha"
    );
    assert!(tarjeta(&a).is_some());
    // Su cabecera lo cierra.
    hacer(&mut e, Accion::CerrarPanel);
    assert!(!panel::ABIERTO.with(|x| x.get()));
    desmontar(e);
}

#[test]
fn un_fichero_de_comentarios_roto_no_se_pisa() {
    let d = Disco::con("{\"comentarios\": [ roto");
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "losa", 0);
    comentar_con(&mut e, "nuevo");
    let md = markdown(&e);
    guardar(&mut e, &md);
    assert_eq!(*d.escrituras.borrow(), 0);
    assert_eq!(
        d.texto.borrow().as_deref(),
        Some("{\"comentarios\": [ roto")
    );
    desmontar(e);
}

#[test]
fn guardar_junta_lo_que_llego_del_movil_mientras_tanto() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    elegir_texto(&e, "losa", 0);
    comentar_con(&mut e, "del PC");
    // La sincronizacion trae un comentario del movil al fichero.
    let mut en_disco = d.leido();
    let t: Vec<u16> = NOTA.encode_utf16().collect();
    let i = NOTA[..NOTA.find("curado").unwrap()].encode_utf16().count();
    let quien = Quien {
        autor: "Teléfono".into(),
        aparato: "MOVI".into(),
    };
    en_disco
        .nuevo(md::ancla_de(&t, i, i + 6).unwrap(), &quien, 5, "del movil")
        .unwrap();
    *d.texto.borrow_mut() = Some(md::escribir(&en_disco));
    // Y aqui se responde al de antes.
    let id = e.comentarios.datos.comentarios[0].id.clone();
    hacer(&mut e, Accion::Responder(id));
    escribir_en_el_cuadro(&e, "respuesta");
    hacer(&mut e, Accion::Enviar);
    let j = d.leido();
    assert_eq!(j.comentarios.len(), 2, "los dos, sin pisar el del movil");
    assert_eq!(j.comentarios[0].respuestas.len(), 1);
    assert_eq!(
        e.comentarios.datos.comentarios.len(),
        2,
        "y el del movil ya se ve aqui"
    );
    desmontar(e);
}

#[test]
fn un_clic_en_el_comentario_lleva_a_su_texto_y_el_cursor_en_el_texto_elige_el_comentario() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    let (a1, _) = elegir_texto(&e, "losa", 0);
    comentar_con(&mut e, "uno");
    let (a2, _) = elegir_texto(&e, "curado", 0);
    comentar_con(&mut e, "dos");
    let (id1, id2) = (
        e.comentarios.datos.comentarios[0].id.clone(),
        e.comentarios.datos.comentarios[1].id.clone(),
    );
    hacer(&mut e, Accion::Tarjeta(id1.clone()));
    assert_eq!(seleccion(e.edit), (a1, a1));
    assert_eq!(e.comentarios.activo.as_deref(), Some(id1.as_str()));
    // El cursor dentro del otro texto comentado elige el otro.
    elegir(e.edit, a2 + 2, a2 + 2);
    let suelta = MSG {
        hwnd: e.edit,
        message: WM_KEYUP,
        ..Default::default()
    };
    seguir(&mut e, &suelta);
    assert_eq!(e.comentarios.activo.as_deref(), Some(id2.as_str()));
    // Fuera de todo texto comentado, no cambia.
    elegir(e.edit, 1, 1);
    seguir(&mut e, &suelta);
    assert_eq!(e.comentarios.activo.as_deref(), Some(id2.as_str()));
    desmontar(e);
}

#[test]
fn abrir_el_panel_deja_el_papel_entero_y_aparta_la_columna_y_cerrarlo_la_devuelve() {
    let d = Disco::default();
    let mut e = abrir(NOTA, &d);
    let ancho =
        |_: &Estado| super::super::VISTA.with(|v| v.borrow().as_ref().unwrap().disp.cuerpo.an);
    // Donde acaba lo escrito por la derecha (el margen de la columna).
    let derecha = |e: &mut Estado| {
        e.estilos.margen = super::super::margen_de(e);
        crate::tabla_ancha::sobra_der_twips()
    };
    let cerrado = ancho(&e);
    let margen = derecha(&mut e);
    alternar_panel(&mut e);
    assert!(panel::ABIERTO.with(|x| x.get()));
    // El papel no pierde nada: las tarjetas van encima de su margen, y la
    // columna se aparta para no ir debajo de ellas.
    assert_eq!(ancho(&e), cerrado);
    let abierto = derecha(&mut e);
    assert!(abierto > margen, "{margen} -> {abierto}");
    let caja = panel::CAJA.with(|c| c.get()).unwrap();
    assert!(caja.derecha() <= cerrado);
    assert!(panel::VISTA.with(|v| v.borrow().is_some()));
    hacer(&mut e, Accion::CerrarPanel);
    assert_eq!(ancho(&e), cerrado);
    assert_eq!(derecha(&mut e), margen);
    assert!(panel::VISTA.with(|v| v.borrow().is_none()));
    desmontar(e);
}

// ---------------------------------------------------------------------------
// Muestras en PNG, a mano:
// `PIXPIN_MUESTRA=<carpeta> cargo test -p pixpin-notas muestra_comentarios -- --ignored`

const NOTA_DE_MUESTRA: &str = "# Objetivos e indicadores\n\
Versión para la segunda asesoría. El **objetivo general** se parte en seis específicos:\n\
\n\
1. **OE1.** Diagnosticar la situación actual del proyecto y establecer su línea base.\n\
2. **OE2.** Validar la problemática identificada mediante juicio de expertos.\n\
3. **OE3.** Diseñar el modelo de gestión de cambios asistido por un agente conversacional.\n\
\n\
## Cómo se cumple cada objetivo\n\
El diagnóstico usa un Diagrama de Pareto con el corte al 80 % y un Ishikawa 8M con cinco porqués. \
La validación se hace con juicio de expertos y una escala Likert de 1 a 5.\n\
\n\
OE3 no lleva indicador porque es un objetivo de entregable; su calidad se comprueba después, en OE5.\n\
\n\
## Tareas\n\
- [x] Pedir la grúa\n\
- [ ] Comprar `cemento 42,5`\n\
> Sin permiso no se corta la calle.\n";

fn muestra_de(claro: bool, nombre: &str, tamano: (i32, i32)) {
    let _com = pixpin_shell::ComDelHilo::iniciar();
    let t: Vec<u16> = NOTA_DE_MUESTRA.encode_utf16().collect();
    let ancla = |que: &str| {
        let i = NOTA_DE_MUESTRA[..NOTA_DE_MUESTRA.find(que).unwrap()]
            .encode_utf16()
            .count();
        md::ancla_de(&t, i, i + que.encode_utf16().count()).unwrap()
    };
    let movil = Quien {
        autor: "Teléfono de Max".into(),
        aparato: "MOVI".into(),
    };
    let pc = Quien {
        autor: "MAXBOOK".into(),
        aparato: "K7Q2".into(),
    };
    let hoy = pixpin_shell::entorno::ahora_utc_ms();
    let mut c = md::Comentarios::default();
    let a = c
        .nuevo(
            ancla("línea base"),
            &pc,
            hoy - 7_200_000,
            "¿La línea base es de 2024 o de este año? Conviene decirlo.",
        )
        .unwrap();
    c.responder(
        &a,
        &movil,
        hoy - 3_600_000,
        "De 2024, con los partes diarios de obra.",
    )
    .unwrap();
    c.responder(&a, &pc, hoy - 600_000, "Perfecto, lo pongo.")
        .unwrap();
    c.nuevo(
        ancla("juicio de expertos"),
        &movil,
        hoy - 5_000_000,
        "¿Cuántos expertos? La asesora pidió al menos cinco.",
    )
    .unwrap();
    c.nuevo(
        ancla("corte al 80 %"),
        &pc,
        hoy - 4_000_000,
        "Citar a Juran para el 80/20.",
    )
    .unwrap();
    let r = c
        .nuevo(
            ancla("Pedir la grúa"),
            &movil,
            hoy - 90_000_000,
            "Hecho el lunes.",
        )
        .unwrap();
    c.resolver(&r, true, &pc, hoy - 80_000_000);
    // Uno cuyo texto ya no esta en la nota.
    let mut perdido = ancla("OE5");
    perdido.cita = "el cronograma de la actividad 9".into();
    perdido.antes = "zzz no esta ".into();
    perdido.despues = " zzz tampoco".into();
    c.nuevo(perdido, &movil, hoy - 100_000_000, "Esto ya no aplica.")
        .unwrap();
    let d = Disco::con(&md::escribir(&c));
    let mut e = abrir_con(NOTA_DE_MUESTRA, de(&d, "MAXBOOK"), claro, tamano);
    // Las tarjetas salen con el boton de la cabecera.
    abrir_panel(&mut e, true);
    elegir_hilo(&mut e, &a, false);
    let arriba = POINT::default();
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    elegir(e.edit, 0, 0);
    pintar(&mut e, None);
    enviar(e.edit, EM_SETSCROLLPOS, 0, &arriba as *const _ as isize);
    componer(&mut e);
    // Respondiendo en la elegida.
    hacer(&mut e, Accion::Responder(a.clone()));
    escribir_en_el_cuadro(&e, "Y añado la fuente");
    let img = muestra(&e);
    let carpeta = std::env::var("PIXPIN_MUESTRA").unwrap_or_else(|_| ".".into());
    let png = pixpin_codec::imagen::codificar_png(&img).unwrap();
    std::fs::write(std::path::Path::new(&carpeta).join(nombre), png).unwrap();
    desmontar(e);
}

#[test]
#[ignore]
fn muestra_comentarios_oscura() {
    muestra_de(false, "nota-md-comentarios-oscura.png", (1180, 960));
    muestra_de(false, "nota-md-comentarios-cajon-oscura.png", (640, 900));
}

#[test]
#[ignore]
fn muestra_comentarios_clara() {
    muestra_de(true, "nota-md-comentarios-clara.png", (1180, 960));
    muestra_de(true, "nota-md-comentarios-cajon-clara.png", (640, 900));
}
