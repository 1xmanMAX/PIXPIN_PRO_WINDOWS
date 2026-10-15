//! Pruebas de editar el lienzo: cada herramienta sobre una escena de verdad,
//! un paso de deshacer por llamada, y los casos que tienen que fallar bien
//! (referencia ambigua, id que no existe, cortar sin cruce...).

use serde_json::{Value, json};

use pixpin_motor2d::Escena;
use pixpin_motor2d::elemento::{ColorRgba, Elemento, Figura};
use pixpin_motor2d::vector::Punto2;

use super::*;

fn raya(a: (f32, f32), b: (f32, f32), color: ColorRgba) -> Elemento {
    Elemento {
        figura: Figura::Linea {
            puntos: vec![Punto2::nuevo(a.0, a.1), Punto2::nuevo(b.0, b.1)],
        },
        x: a.0.min(b.0),
        y: a.1.min(b.1),
        ancho: (a.0 - b.0).abs(),
        alto: (a.1 - b.1).abs(),
        trazo: color,
        ..Default::default()
    }
}

fn texto(t: &str, x: f32, y: f32, tam: f32) -> Elemento {
    Elemento {
        figura: Figura::Texto {
            texto: t.into(),
            tam,
            familia: "Segoe UI".into(),
        },
        x,
        y,
        ancho: t.len() as f32 * tam * 0.5,
        alto: tam * 1.25,
        ..Default::default()
    }
}

fn circulo(cx: f32, cy: f32, r: f32, color: ColorRgba) -> Elemento {
    Elemento {
        figura: Figura::Elipse,
        x: cx - r,
        y: cy - r,
        ancho: 2.0 * r,
        alto: 2.0 * r,
        trazo: color,
        ..Default::default()
    }
}

const ROJO: ColorRgba = ColorRgba::opaco(0.88, 0.19, 0.19);
const AZUL: ColorRgba = ColorRgba::opaco(0.10, 0.44, 0.76);
const NEGRO: ColorRgba = ColorRgba::opaco(0.12, 0.12, 0.12);

/// Valida y aplica una llamada como la haria el modelo.
fn llamar(escena: &mut Escena, ctx: &Contexto, nombre: &str, args: Value) -> Result<Hecho, String> {
    llamar_con(escena, ctx, nombre, args, &mut SinImagenes)
}

fn llamar_con(escena: &mut Escena, ctx: &Contexto, nombre: &str, args: Value, an: &mut dyn Anfitrion) -> Result<Hecho, String> {
    let f = fila(nombre).expect("existe");
    let e = validar(f, args.as_object().unwrap()).map_err(|e| e.to_string())?;
    aplicar(escena, &e, ctx, an)
}

/// Lo visible, para comparar antes y despues.
fn foto(escena: &Escena) -> Vec<Elemento> {
    escena
        .visibles()
        .cloned()
        .map(|mut e| {
            e.version = 0;
            e
        })
        .collect()
}

/// Aplica y comprueba que un solo `deshacer` lo devuelve todo como estaba.
fn un_paso(escena: &mut Escena, ctx: &Contexto, nombre: &str, args: Value) -> Hecho {
    let antes = foto(escena);
    let h = llamar(escena, ctx, nombre, args.clone()).unwrap_or_else(|e| panic!("{nombre} {args}: {e}"));
    assert_ne!(foto(escena), antes, "{nombre} no cambio nada");
    let mut copia = escena.clone();
    assert!(copia.deshacer());
    assert_eq!(foto(&copia), antes, "{nombre}: un deshacer no lo devolvio todo");
    h
}

fn cruce() -> (Escena, u64, u64) {
    let mut s = Escena::nueva();
    // Una horizontal arriba y una diagonal abajo que la cruza.
    let h = s.anadir(raya((0.0, 100.0), (300.0, 100.0), ROJO));
    let d = s.anadir(raya((100.0, 0.0), (200.0, 300.0), AZUL));
    (s, h, d)
}

#[test]
fn la_tabla_genera_esquemas_validos_y_valida_lo_obligatorio() {
    for h in herramientas() {
        assert_eq!(h.parametros["type"], "object", "{}", h.nombre);
        for r in h.parametros["required"].as_array().unwrap() {
            assert!(h.parametros["properties"][r.as_str().unwrap()].is_object());
        }
    }
    // Negativos: falta lo obligatorio, opcion que no existe.
    let f = fila(CORTAR).unwrap();
    assert!(validar(f, json!({}).as_object().unwrap()).is_err());
    let t = fila(TRANSFORMAR).unwrap();
    let e = validar(t, json!({"que": "#1", "accion": "teletransportar"}).as_object().unwrap()).unwrap_err();
    assert!(e.to_string().contains("mover"), "{e}");
    // Tolerante: numeros como texto y una lista de ids.
    let ok = validar(t, json!({"que": [3, 4], "accion": "Mover", "dx": "10"}).as_object().unwrap()).unwrap();
    assert_eq!(ok.texto("que"), Some("#3 #4"));
    assert_eq!(ok.numero("dx"), Some(10.0));
    assert_eq!(ok.texto("accion"), Some("mover"));
    // Y pasan por la validacion comun de herramientas.
    let cruda = crate::motor::LlamadaCruda {
        id: "1".into(),
        nombre: CORTAR.into(),
        argumentos: r##"{"que":"#2"}"##.into(),
    };
    let v = crate::herramientas::validar(&cruda, &crate::herramientas::catalogo()).unwrap();
    assert_eq!(v.llamada.nombre(), CORTAR);
}

#[test]
fn el_resumen_dice_ids_tipos_colores_y_textos_en_el_marco() {
    let (mut s, h, _) = cruce();
    s.anadir(texto("Total: 42", 10.0, 200.0, 20.0));
    let ctx = Contexto {
        marco: (-50.0, 0.0),
        elegidos: vec![h],
        ..Default::default()
    };
    let r = resumen(&s, &ctx);
    assert!(r.contains("3 elementos"), "{r}");
    assert!(r.contains(&format!("Elegido por el usuario: #{h}")), "{r}");
    assert!(r.contains(&format!("#{h} recta rojo (50,100)->(350,100)")), "{r}");
    assert!(r.contains("texto negro") && r.contains("\"Total: 42\""), "{r}");
    assert_eq!(resumen(&Escena::nueva(), &Contexto::default()), "Lienzo destino: vacio.");
}

#[test]
fn el_resumen_se_recorta_con_muchos_elementos() {
    let mut s = Escena::nueva();
    for i in 0..(MAX_RESUMEN + 10) {
        s.anadir(raya((0.0, i as f32), (10.0, i as f32), NEGRO));
    }
    let r = resumen(&s, &Contexto::default());
    assert_eq!(r.lines().count(), 1 + MAX_RESUMEN + 1, "{r}");
    assert!(r.ends_with("(+10 mas antiguos sin listar)"));
}

#[test]
fn referencias_por_id_tipo_color_texto_y_sitio() {
    let (mut s, h, d) = cruce();
    let t = s.anadir(texto("Resultado: 12", 0.0, 400.0, 16.0));
    let titulo = s.anadir(texto("Mi plano", 0.0, 0.0, 32.0));
    let c = s.anadir(circulo(500.0, 500.0, 30.0, ROJO));
    let ctx = Contexto::default();
    let r = |q: &str| referencia::resolver(&s, q, &ctx, None).map(|e| e.ids);
    assert_eq!(r(&format!("#{h}")).unwrap(), vec![h]);
    assert_eq!(r(&format!("{d}")).unwrap(), vec![d]);
    assert_eq!(r("el circulo rojo").unwrap(), vec![c]);
    assert_eq!(r("la recta azul").unwrap(), vec![d]);
    assert_eq!(r("el texto \"resultado\"").unwrap(), vec![t]);
    assert_eq!(r("el titulo").unwrap(), vec![titulo]);
    assert_eq!(r("la recta de arriba").unwrap(), vec![h]);
    assert_eq!(referencia::varios(&s, "las rectas", &ctx).unwrap(), vec![h, d]);
    // Lo ultimo: lo que toco la llamada anterior, o lo de encima.
    assert_eq!(r("lo ultimo").unwrap(), vec![c]);
    let ctx2 = Contexto { ultimos: vec![h], ..Default::default() };
    assert_eq!(referencia::resolver(&s, "lo ultimo", &ctx2, None).unwrap().ids, vec![h]);
    // Por zona.
    let zona = referencia::resolver(&s, "todo", &ctx, Some((400.0, 400.0, 200.0, 200.0))).unwrap();
    assert_eq!(zona.ids, vec![c]);
}

#[test]
fn una_referencia_ambigua_devuelve_la_lista_corta() {
    let (s, h, d) = cruce();
    let ctx = Contexto::default();
    let e = referencia::uno(&s, "la recta", &ctx, None).unwrap_err();
    assert!(e.contains("ambiguo") && e.contains(&format!("#{h}")) && e.contains(&format!("#{d}")), "{e}");
    let e = referencia::varios(&s, "la recta", &ctx).unwrap_err();
    assert!(e.contains("ambiguo"), "{e}");
    // Y cortar con una referencia ambigua no toca la escena.
    let mut s2 = s.clone();
    let antes = foto(&s2);
    assert!(llamar(&mut s2, &ctx, CORTAR, json!({"que": "la recta"})).is_err());
    assert_eq!(foto(&s2), antes);
}

#[test]
fn un_id_que_no_existe_o_nada_elegido_es_un_error_con_lo_que_hay() {
    let (s, h, _) = cruce();
    let ctx = Contexto::default();
    let e = referencia::resolver(&s, "#999", &ctx, None).unwrap_err();
    assert!(e.contains("no existe #999") && e.contains(&format!("#{h}")), "{e}");
    let e = referencia::resolver(&s, "lo elegido", &ctx, None).unwrap_err();
    assert!(e.contains("nada elegido"), "{e}");
    let e = referencia::resolver(&s, "el triangulo verde", &ctx, None).unwrap_err();
    assert!(e.contains("no hay nada que encaje"), "{e}");
    assert!(referencia::resolver(&Escena::nueva(), "#1", &ctx, None).unwrap_err().contains("vacio"));
}

#[test]
fn cortar_la_de_abajo_en_el_cruce_en_un_paso() {
    let (mut s, h, d) = cruce();
    let ctx = Contexto::default();
    // La diagonal cruza la horizontal en (133,100): quitar su trozo de
    // abajo (el fin, el mas largo).
    let hecho = un_paso(&mut s, &ctx, CORTAR, json!({"que": format!("#{d}"), "con": format!("#{h}"), "quitar": "fin"}));
    assert_eq!(hecho.ids, vec![d]);
    let p = s.buscar(d).unwrap().puntos().unwrap().to_vec();
    assert!((p[1].y - 100.0).abs() < 0.5, "acaba en el cruce: {p:?}");
    assert!(p.iter().all(|q| q.y <= 100.5));
    // Sin «quitar»: el trozo mas corto de un extremo (el de arriba de la
    // diagonal mide 105; el de abajo, 211).
    let (mut s, _, d) = cruce();
    llamar(&mut s, &ctx, CORTAR, json!({"que": "la recta azul"})).unwrap();
    let p = s.buscar(d).unwrap().puntos().unwrap().to_vec();
    assert!((p[0].y - 100.0).abs() < 0.5 && (p[1].y - 300.0).abs() < 0.5, "{p:?}");
}

#[test]
fn cortar_sin_interseccion_falla_sin_tocar_nada() {
    let mut s = Escena::nueva();
    let a = s.anadir(raya((0.0, 0.0), (100.0, 0.0), NEGRO));
    s.anadir(raya((0.0, 50.0), (100.0, 50.0), NEGRO));
    let antes = foto(&s);
    let e = llamar(&mut s, &Contexto::default(), CORTAR, json!({"que": format!("#{a}")})).unwrap_err();
    assert!(e.contains("no se cruza"), "{e}");
    assert_eq!(foto(&s), antes);
    assert!(!s.hay_que_deshacer() || s.pasos_cerrados() == 2);
}

#[test]
fn extender_hasta_otra_y_sin_nada_delante_falla() {
    let mut s = Escena::nueva();
    let corta = s.anadir(raya((0.0, 50.0), (60.0, 50.0), NEGRO));
    let pared = s.anadir(raya((200.0, 0.0), (200.0, 100.0), NEGRO));
    let ctx = Contexto::default();
    un_paso(&mut s, &ctx, EXTENDER, json!({"que": format!("#{corta}"), "hasta": format!("#{pared}")}));
    let p = s.buscar(corta).unwrap().puntos().unwrap().to_vec();
    assert!((p[1].x - 200.0).abs() < 0.5, "{p:?}");
    // Negativo: la pared, hacia arriba, no topa con nada.
    let e = llamar(&mut s, &ctx, EXTENDER, json!({"que": format!("#{pared}"), "hasta": format!("#{corta}"), "extremo": "inicio"})).unwrap_err();
    assert!(e.contains("no topa"), "{e}");
}

#[test]
fn geometria_exacta_en_el_marco_del_modelo() {
    let mut s = Escena::nueva();
    let ctx = Contexto {
        marco: (1000.0, 2000.0),
        ..Default::default()
    };
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "recta", "puntos": [10, 20, 110, 20], "color": "rojo", "grosor": 3}));
    let r = s.buscar(h.ids[0]).unwrap();
    assert_eq!(r.puntos().unwrap(), &[Punto2::nuevo(1010.0, 2020.0), Punto2::nuevo(1110.0, 2020.0)]);
    assert_eq!(r.grosor, 3.0);
    assert_eq!(nombre_de_color(r.trazo), "rojo");
    // Por angulo y longitud: 90 grados es hacia arriba.
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "recta", "desde": [0, 100], "angulo": 90, "longitud": 50}));
    let p = s.buscar(h.ids[0]).unwrap().puntos().unwrap().to_vec();
    assert!((p[1].x - 1000.0).abs() < 1e-3 && (p[1].y - 2050.0).abs() < 1e-3, "{p:?}");
    // Circulo por tres puntos: (0,0), (100,0), (50,50) -> centro (50,0), r 50.
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "circulo", "puntos": [0, 0, 100, 0, 50, 50]}));
    let c = s.buscar(h.ids[0]).unwrap();
    assert!(matches!(c.figura, Figura::Elipse));
    assert!((c.x - 1000.0).abs() < 1e-3 && (c.ancho - 100.0).abs() < 1e-3, "{c:?}");
    // Negativo: tres puntos en linea.
    let e = llamar(&mut s, &ctx, GEOMETRIA, json!({"figura": "circulo", "puntos": [0, 0, 10, 0, 20, 0]})).unwrap_err();
    assert!(e.contains("en linea"), "{e}");
    // Arco de 0 a 90: un cuarto, por arriba a la derecha.
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "arco", "centro": [0, 0], "radio": 40, "inicio": 0, "fin": 90}));
    match s.buscar(h.ids[0]).unwrap().figura {
        Figura::Arco { inicio, barrido } => {
            assert_eq!(inicio, 0.0);
            assert!((barrido.unwrap() + std::f32::consts::FRAC_PI_2).abs() < 1e-4);
        }
        ref otra => panic!("{otra:?}"),
    }
}

#[test]
fn perpendicular_y_paralela_a_una_recta_que_ya_esta() {
    let mut s = Escena::nueva();
    let base = s.anadir(raya((0.0, 0.0), (100.0, 0.0), NEGRO));
    let ctx = Contexto::default();
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "perpendicular", "a": format!("#{base}"), "por": [30, 40]}));
    let p = s.buscar(h.ids[0]).unwrap().puntos().unwrap().to_vec();
    assert_eq!(p, vec![Punto2::nuevo(30.0, 40.0), Punto2::nuevo(30.0, 0.0)]);
    let h = un_paso(&mut s, &ctx, GEOMETRIA, json!({"figura": "paralela", "a": format!("#{base}"), "distancia": 20}));
    let p = s.buscar(h.ids[0]).unwrap().puntos().unwrap().to_vec();
    assert!(p.iter().all(|q| (q.y + 20.0).abs() < 1e-3), "a la izquierda (arriba) de la recta: {p:?}");
    // Negativo: la base no es una recta.
    let c = s.anadir(circulo(0.0, 0.0, 10.0, NEGRO));
    let e = llamar(&mut s, &ctx, GEOMETRIA, json!({"figura": "paralela", "a": format!("#{c}"), "distancia": 5})).unwrap_err();
    assert!(e.contains("recta"), "{e}");
}

#[test]
fn trazos_a_mano_con_el_gesto_del_editor() {
    let mut s = Escena::nueva();
    let ctx = Contexto::default();
    let h = un_paso(&mut s, &ctx, TRAZAR, json!({"herramienta": "grafito", "dureza": "8B", "puntos": [0, 0, 100, 30, 200, 0]}));
    let g = s.buscar(h.ids[0]).unwrap();
    assert!(matches!(g.figura, Figura::Lapiz { .. }));
    assert!(g.material.es_grafito());
    assert_eq!(g.extras.dureza.map(|d| d.rotulo()), Some("8B"));
    // Muchos puntos repartidos como una mano, y sin presiones (las simula la
    // tinta, como con el raton).
    if let Figura::Lapiz { puntos, presiones, .. } = &g.figura {
        assert!(puntos.len() > 20);
        assert!(presiones.is_empty());
    }
    let h = un_paso(&mut s, &ctx, TRAZAR, json!({"herramienta": "lapiz", "puntos": [0, 50, 100, 50], "color": "azul"}));
    assert_eq!(nombre_de_color(s.buscar(h.ids[0]).unwrap().trazo), "azul");
    // Negativo: sin puntos ni «sobre».
    assert!(llamar(&mut s, &ctx, TRAZAR, json!({"herramienta": "lapiz"})).is_err());
}

#[test]
fn resaltar_y_subrayar_un_texto() {
    let mut s = Escena::nueva();
    let t = s.anadir(texto("Resultado final", 100.0, 100.0, 24.0));
    let ctx = Contexto::default();
    let h = un_paso(&mut s, &ctx, TRAZAR, json!({"herramienta": "resaltador", "sobre": "el texto \"resultado\""}));
    let r = s.buscar(h.ids[0]).unwrap();
    assert!(matches!(r.figura, Figura::Resaltador { .. }));
    assert_eq!(nombre_de_color(r.trazo), "amarillo");
    let (tx0, ty0, tx1, ty1) = s.buscar(t).unwrap().caja();
    let (rx0, ry0, rx1, ry1) = r.caja();
    assert!(rx0 <= tx0 + 2.0 && rx1 >= tx1 - 2.0, "cubre lo ancho");
    assert!(ry0 < ty1 && ry1 > ty0, "pasa por encima del texto");
    // Subrayar con el lapiz: por debajo.
    let h = un_paso(&mut s, &ctx, TRAZAR, json!({"herramienta": "lapiz", "sobre": format!("#{t}"), "como": "subrayar", "color": "rojo"}));
    let (_, y0, _, _) = s.buscar(h.ids[0]).unwrap().caja();
    assert!(y0 > ty1 - 6.0, "{y0} {ty1}");
}

/// Un anfitrion de prueba: una imagen de 800x400.
struct ConFoto;

impl Anfitrion for ConFoto {
    fn imagen_nueva(&mut self, fuente: &str) -> Result<Elemento, String> {
        if fuente == "nada" {
            return Err("no hay ninguna imagen adjunta".into());
        }
        Ok(Elemento {
            figura: Figura::Imagen { id_objeto: 77 },
            ancho: 800.0,
            alto: 400.0,
            ..Default::default()
        })
    }
    fn tamano_de(&self, _: u64) -> Option<(u32, u32)> {
        Some((800, 400))
    }
}

#[test]
fn pegar_la_foto_y_recortar_la_mitad_izquierda() {
    let mut s = Escena::nueva();
    let ctx = Contexto::default();
    let h = llamar_con(&mut s, &ctx, IMAGEN, json!({"accion": "pegar", "caja": [0, 0, 400]}), &mut ConFoto).unwrap();
    let id = h.ids[0];
    assert_eq!((s.buscar(id).unwrap().ancho, s.buscar(id).unwrap().alto), (400.0, 200.0));
    let ctx = Contexto {
        ultimos: h.ids.clone(),
        ..Default::default()
    };
    // «Quita la mitad izquierda»: se conserva la derecha.
    let antes = foto(&s);
    llamar_con(&mut s, &ctx, IMAGEN, json!({"accion": "recortar", "quitar": "izquierda", "cuanto": 0.5}), &mut ConFoto).unwrap();
    let e = s.buscar(id).unwrap();
    assert_eq!((e.x, e.ancho), (200.0, 200.0));
    let r = e.extras.recorte.unwrap();
    assert_eq!((r.x, r.ancho, r.ancho_natural), (400.0, 400.0, 800.0));
    let mut c = s.clone();
    c.deshacer();
    assert_eq!(foto(&c), antes);
    // Negativos: sin imagen que pegar; recortar algo que no es imagen.
    assert!(llamar_con(&mut s, &ctx, IMAGEN, json!({"accion": "pegar", "imagen": "nada"}), &mut ConFoto).is_err());
    let l = s.anadir(raya((0.0, 0.0), (10.0, 0.0), NEGRO));
    let e = llamar_con(&mut s, &ctx, IMAGEN, json!({"accion": "recortar", "que": format!("#{l}"), "quitar": "arriba"}), &mut ConFoto).unwrap_err();
    assert!(e.contains("imagen"), "{e}");
    assert!(llamar(&mut Escena::nueva(), &ctx, IMAGEN, json!({"accion": "pegar"})).is_err());
}

#[test]
fn mover_escalar_girar_duplicar_borrar_y_estilo() {
    let (mut s, h, d) = cruce();
    let ctx = Contexto::default();
    un_paso(&mut s, &ctx, TRANSFORMAR, json!({"que": format!("#{h}"), "accion": "mover", "dx": 10, "dy": 5}));
    assert_eq!(s.buscar(h).unwrap().puntos().unwrap()[0], Punto2::nuevo(10.0, 105.0));
    let c = s.anadir(circulo(500.0, 500.0, 20.0, ROJO));
    un_paso(&mut s, &ctx, TRANSFORMAR, json!({"que": "el circulo rojo", "accion": "escalar", "factor": 2}));
    let e = s.buscar(c).unwrap();
    assert!((e.ancho - 80.0).abs() < 1e-3 && (e.x - 460.0).abs() < 1e-3, "desde su centro: {e:?}");
    un_paso(&mut s, &ctx, TRANSFORMAR, json!({"que": format!("#{d}"), "accion": "girar", "grados": 90}));
    let dup = un_paso(&mut s, &ctx, TRANSFORMAR, json!({"que": format!("#{c}"), "accion": "duplicar", "dx": 100, "dy": 0}));
    assert_eq!(dup.ids.len(), 1);
    assert_ne!(dup.ids[0], c);
    un_paso(&mut s, &ctx, ESTILO, json!({"que": format!("#{c}"), "color": "verde", "relleno": "amarillo", "trazo": "discontinuo"}));
    let e = s.buscar(c).unwrap();
    assert_eq!(nombre_de_color(e.trazo), "verde");
    assert!(e.relleno.is_some());
    un_paso(&mut s, &ctx, TRANSFORMAR, json!({"que": format!("#{c}"), "accion": "borrar"}));
    assert!(s.buscar(c).is_none_or(|e| e.borrado));
    // Negativo: estilo sin nada que cambiar.
    assert!(llamar(&mut s, &ctx, ESTILO, json!({"que": format!("#{h}")})).is_err());
}

#[test]
fn seleccionar_devuelve_ids_para_encadenar() {
    let (mut s, h, d) = cruce();
    let ctx = Contexto::default();
    let pasos = s.pasos_cerrados();
    let r = llamar(&mut s, &ctx, SELECCIONAR, json!({"que": "las rectas"})).unwrap();
    assert_eq!(r.ids, vec![h, d]);
    assert!(r.texto.contains(&format!("#{h} recta")), "{}", r.texto);
    assert_eq!(s.pasos_cerrados(), pasos, "elegir no cambia el dibujo");
}

#[test]
fn el_resumen_va_solo_cuando_viene_a_cuento_y_al_final_del_pedido() {
    assert!(viene_a_cuento("resalta el titulo"));
    assert!(viene_a_cuento("dibuja un circulo rojo"));
    assert!(viene_a_cuento("¿que hay en el lienzo?"));
    // Negativos: tareas y notas no lo llevan.
    assert!(!viene_a_cuento("apunta la tarea comprar pan"));
    assert!(!viene_a_cuento("hola"));
    let mut h = vec![crate::motor::Mensaje::sistema("s"), crate::motor::Mensaje::usuario("corta la recta")];
    anadir_resumen(&mut h, "Lienzo destino: vacio.");
    assert_eq!(h[1].texto_entero(), "corta la recta\n\n[Lienzo destino: vacio.]");
    assert_eq!(h[0].texto_entero(), "s");
}

#[test]
fn lo_que_dibuja_va_antes_que_lo_que_edita_en_la_misma_respuesta() {
    use crate::herramientas::{Llamada, validar};
    let l = |n: &str, a: &str| {
        validar(
            &crate::motor::LlamadaCruda { id: "x".into(), nombre: n.into(), argumentos: a.into() },
            &crate::herramientas::catalogo(),
        )
        .unwrap()
        .llamada
    };
    let llamadas: Vec<Llamada> = vec![
        l(CORTAR, r#"{"que":"la recta de abajo"}"#),
        l("dibujar_en_lienzo", r#"{"elementos":[{"tipo":"linea","puntos":[0,0,10,10]}]}"#),
    ];
    assert_eq!(orden_de_ejecucion(&llamadas), vec![1, 0]);
    let mut hechas = Vec::new();
    let r = en_orden(&llamadas, |x| {
        hechas.push(x.nombre());
        x.nombre()
    });
    assert_eq!(hechas, vec!["dibujar_en_lienzo", CORTAR]);
    assert_eq!(r, vec![CORTAR, "dibujar_en_lienzo"], "los resultados, en el orden de la respuesta");
}

#[test]
fn lo_que_falta_se_saca_del_pedido_sin_pisar_lo_puesto() {
    let f = fila(TRAZAR).unwrap();
    let mut e = validar(f, json!({"herramienta": "grafito", "sobre": "#1"}).as_object().unwrap()).unwrap();
    ajustar_al_pedido(&mut e, "Rodea el rectángulo con grafito 8B en azul");
    assert_eq!(e.texto("como"), Some("rodear"));
    assert_eq!(e.texto("dureza"), Some("8B"));
    assert_eq!(e.texto("color"), Some("azul"));
    // Lo que el modelo puso no se toca; sin pistas, nada.
    let mut e = validar(f, json!({"herramienta": "lapiz", "sobre": "#1", "como": "tachar"}).as_object().unwrap()).unwrap();
    ajustar_al_pedido(&mut e, "subraya esto");
    assert_eq!(e.texto("como"), Some("tachar"));
    let mut e = validar(f, json!({"herramienta": "lapiz", "puntos": [0, 0, 1, 1]}).as_object().unwrap()).unwrap();
    ajustar_al_pedido(&mut e, "haz un trazo");
    assert!(e.texto("como").is_none() && e.texto("dureza").is_none() && e.texto("color").is_none());
    // Recortar sin decir que: del pedido.
    let fi = fila(IMAGEN).unwrap();
    let mut e = validar(fi, json!({"accion": "recortar"}).as_object().unwrap()).unwrap();
    ajustar_al_pedido(&mut e, "pega la foto y recorta la mitad izquierda");
    assert_eq!(e.numeros("conservar"), Some(vec![0.0, 0.0, 0.5, 1.0]));
    let mut e = validar(fi, json!({"accion": "recortar"}).as_object().unwrap()).unwrap();
    ajustar_al_pedido(&mut e, "quita la parte de arriba de la foto");
    assert_eq!(e.texto("quitar"), Some("arriba"));
}

#[test]
fn una_caja_copiada_del_resumen_o_un_numero_de_orden_tambien_eligen() {
    let mut s = Escena::nueva();
    let a = s.anadir(raya((0.0, 0.0), (100.0, 0.0), NEGRO));
    let b = s.anadir(raya((0.0, 50.0), (100.0, 50.0), NEGRO));
    let r = s.anadir(Elemento { x: 200.0, y: 150.0, ancho: 160.0, alto: 100.0, ..Default::default() });
    let ctx = Contexto::default();
    // [200,150,160,100] llega como «#200 #150 #160 #100».
    assert_eq!(referencia::resolver(&s, "#200 #150 #160 #100", &ctx, None).unwrap().ids, vec![r]);
    assert_eq!(referencia::resolver(&s, "recta_2", &ctx, None).unwrap().ids, vec![b]);
    assert_eq!(referencia::resolver(&s, &format!("la recta {a}"), &ctx, None).unwrap().ids, vec![a]);
    // Negativo: cuatro numeros que no encierran nada siguen siendo ids que no existen.
    assert!(referencia::resolver(&s, "#900 #900 #5 #5", &ctx, None).unwrap_err().contains("no existe"));
}
