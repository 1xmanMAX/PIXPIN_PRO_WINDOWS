//! Pruebas sobre una carpeta de datos de mentira (con todas las clases de
//! mensaje y dos listas de tareas) y sobre el dialogo JSON-RPC con Flow.

use crate::datos::Datos;
use crate::pedido::{Envio, Mensajero};
use crate::resultados::{resultados, Accion, Contexto, Resultado};
use crate::rpc::{bucle, Plugin};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

/// Una raiz de datos de PixPin, nueva para cada prueba.
fn raiz(etiqueta: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!("pixpin-lanzador-pruebas-{etiqueta}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&r);
    let p = r.join("proyectos");
    fs::create_dir_all(&p).unwrap();
    fs::write(
        p.join("indice.json"),
        json!({ "proyectos": [
            { "id": "G1", "nombre": "nombre raro del movil", "tocado": 3000, "hojas": 0, "guardados": true },
            { "id": "P1", "nombre": "Gestión de proyectos", "tocado": 5000, "hojas": 2 },
            { "id": "P2", "nombre": "Thesis", "tocado": 4000, "hojas": 0, "campoNuevo": [1, 2] },
            { "id": "P3", "nombre": "Sin carpeta", "tocado": 9000 },
            { "id": "PX", "nombre": "Tirado", "tocado": 9000, "papelera": true },
            { "id": "P4", "nombre": "Chat de clase", "tocado": 100 }
        ]})
        .to_string(),
    )
    .unwrap();
    for d in ["G1/android/guardados", "P1/archivos", "P1/notas", "P2", "PX", "P4"] {
        fs::create_dir_all(p.join(d)).unwrap();
    }
    fs::write(p.join("G1/android/guardados/foto.jpg"), b"jpg").unwrap();
    fs::write(p.join("G1/android/guardados/voz_1.m4a"), b"m4a").unwrap();
    fs::write(p.join("P1/archivos/Objetivos.md"), b"# Objetivos").unwrap();
    fs::write(p.join("P1/notas/suelta.md"), "\u{feff}\n# Apuntes sueltos\n\ntexto").unwrap();
    // La nota de verdad del mensaje n2 (por su referencia), una foto y un audio.
    fs::write(p.join("P1/notas/zacta.md"), "# Acta\n\nreunión").unwrap();
    fs::write(p.join("P1/archivos/croquis.png"), b"png").unwrap();
    fs::write(p.join("P1/archivos/v.m4a"), b"m4a").unwrap();
    // El icono de «md» ya lo pinto la app; el de «pdf», no.
    fs::create_dir_all(r.join("cache/iconos-de-extension")).unwrap();
    fs::write(r.join("cache/iconos-de-extension/md.png"), b"png").unwrap();
    fs::write(p.join("P4/guardados.jsonl"), "").unwrap();
    let lineas = |v: &[Value]| v.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let g1 = lineas(&[
        json!({ "id": "n1", "cuando": 100, "clase": "NOTA", "texto": "# Idea brillante\n\ncomprar pan integral", "nombre": "" }),
        json!({ "id": "i1", "cuando": 101, "clase": "IMAGEN", "ruta": "pixpin:files/guardados/foto.jpg", "nombre": "foto.jpg" }),
        json!({ "id": "v1", "cuando": 102, "clase": "VOZ", "ruta": "pixpin:files/guardados/voz_1.m4a", "nombre": "Clase de cálculo", "transcripcion": "integrales dobles" }),
        json!({ "id": "v2", "cuando": 103, "clase": "VOZ", "ruta": "pixpin:files/guardados/no_esta.m4a", "nombre": "" }),
        json!({ "id": "t1", "cuando": 104, "clase": "MINIAPP", "miniapp": "tareas", "texto": "# Compra\n\n- [ ] pan\n- [x] leche\n- [ ] huevos" }),
        json!({ "id": "ib1", "cuando": 105, "clase": "IMAGEN", "nombre": "buzon.png", "enBuzon": true }),
    ]) + "\n{roto\n";
    fs::write(p.join("G1/guardados.jsonl"), g1).unwrap();
    let p1 = lineas(&[
        json!({ "id": "d1", "cuando": 200, "clase": "DIBUJO", "referencia": "dib-1", "nombre": "Lienzo" }),
        json!({ "id": "pg1", "cuando": 201, "clase": "PAGINA", "referencia": "pag-1", "nombre": "Página uno" }),
        json!({ "id": "a1", "cuando": 202, "clase": "ARCHIVO", "ruta": "archivos/Objetivos.md", "nombre": "Objetivos e Indicadores.md" }),
        json!({ "id": "a2", "cuando": 203, "clase": "ARCHIVO", "ruta": "/storage/emulated/0/x.pdf", "nombre": "Remoto.pdf" }),
        json!({ "id": "pp1", "cuando": 204, "clase": "PROYECTO", "nombre": "otro", "referencia": "P2" }),
        json!({ "id": "t2", "cuando": 205, "clase": "MINIAPP", "miniapp": "tareas", "texto": "# Compra\n- [ ] tornillos" }),
        json!({ "id": "m3", "cuando": 206, "clase": "MINIAPP", "miniapp": "contador", "texto": "# Vasos\nvalor: 3" }),
        json!({ "id": "n2", "cuando": 207, "clase": "NOTA", "texto": "# Acta\n\nreunión", "referencia": "zacta" }),
        json!({ "id": "x1", "cuando": 208, "clase": "NOTA", "texto": "hola equipo, mañana a las 9\nsegunda línea" }),
        json!({ "id": "i2", "cuando": 209, "clase": "IMAGEN", "ruta": "archivos/croquis.png", "nombre": "croquis.png" }),
        json!({ "id": "v3", "cuando": 150, "clase": "VOZ", "ruta": "archivos/v.m4a", "nombre": "",
            "transcripcion": "revisar el presupuesto", "duracionMs": 75000 }),
        json!({ "id": "b9", "cuando": 999, "clase": "NOTA", "texto": "del buzon", "enBuzon": true, "duracionMs": null }),
    ]);
    fs::write(p.join("P1/guardados.jsonl"), p1).unwrap();
    fs::write(
        p.join("P1/proyecto.json"),
        json!({ "id": "P1", "hojas": [
            { "id": "h1", "nombre": "Plano de la casa", "dibujo": "dib-1" },
            { "id": "h2", "nombre": "Hoja suelta", "dibujo": "dib-2" },
            { "id": "h3", "nombre": "", "dibujo": null, "pagina": 0 }
        ]})
        .to_string(),
    )
    .unwrap();
    fs::write(p.join("P2/guardados.jsonl"), "").unwrap();
    r
}

fn ctx() -> Contexto {
    Contexto::con("pp ", 1000)
}

fn buscar(r: &Path, q: &str) -> Vec<Resultado> {
    let mut d = Datos::nuevo(r.to_path_buf());
    resultados(&d.proyectos(), q, &ctx())
}

fn pedido_de(r: &Resultado) -> &Value {
    match &r.accion {
        Accion::Pedido(p) | Accion::PedirYSeguir { pedido: p, .. } => p,
        otra => panic!("no es un pedido: {otra:?}"),
    }
}

// --- Leer y buscar ---------------------------------------------------------

#[test]
fn lee_los_proyectos_sin_papelera_ni_carpetas_que_faltan() {
    let r = raiz("proyectos");
    let mut d = Datos::nuevo(r.clone());
    let ps = d.proyectos();
    let ids: Vec<&str> = ps.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["G1", "P1", "P2", "P4"]);
    assert_eq!(ps[0].nombre, "Mensajes guardados");
    // La linea rota y la del buzon no cuentan.
    assert_eq!(ps[0].mensajes.len(), 5);
    assert_eq!(ps[1].hojas_json.len(), 3);
    assert_eq!(ps[1].notas_md[0].titulo, "Apuntes sueltos");
}

#[test]
fn la_cache_se_pone_al_dia_cuando_cambia_el_fichero() {
    let r = raiz("cache");
    let mut d = Datos::nuevo(r.clone());
    assert_eq!(d.proyectos()[2].mensajes.len(), 0);
    // Se adelanta la cache: dura mientras el fichero no cambie.
    assert!(d.retocar("G1", "t1", |t| crate::datos::con_tarea_anadida(t, "yogur")));
    assert!(d.proyectos()[0].mensajes.iter().any(|m| m.texto().contains("yogur")));
    std::thread::sleep(std::time::Duration::from_millis(30));
    fs::write(
        r.join("proyectos/P2/guardados.jsonl"),
        json!({ "id": "x", "cuando": 1, "clase": "NOTA", "texto": "nueva" }).to_string(),
    )
    .unwrap();
    let ps = d.proyectos();
    assert_eq!(ps[2].mensajes.len(), 1);
    assert!(ps[0].mensajes.iter().any(|m| m.texto().contains("yogur")), "G1 no cambio: sigue lo adelantado");
}

#[test]
fn un_archivo_se_abre_por_su_ruta_real() {
    let r = raiz("archivo");
    let v = buscar(&r, "objetivos");
    assert_eq!(v[0].titulo, "Objetivos e Indicadores.md");
    assert!(v[0].subtitulo.starts_with("Archivo · Gestión de proyectos"), "{}", v[0].subtitulo);
    let p = pedido_de(&v[0]);
    assert_eq!(p["accion"], "abrir");
    assert_eq!(p["que"]["tipo"], "fichero");
    assert_eq!(p["que"]["ruta"], r.join("proyectos").join("P1").join("archivos").join("Objetivos.md").to_string_lossy().as_ref());
}

#[test]
fn lo_que_no_esta_en_el_equipo_abre_su_mensaje() {
    let r = raiz("remoto");
    let v = buscar(&r, "remoto");
    assert!(v[0].subtitulo.contains("no está en este equipo"));
    assert_eq!(pedido_de(&v[0])["que"], json!({ "tipo": "mensaje", "proyecto": "P1", "codigo": "a2" }));
}

#[test]
fn lienzos_por_el_nombre_de_su_hoja_y_hojas_sin_mensaje() {
    let r = raiz("lienzos");
    let v = buscar(&r, "plano");
    assert_eq!(v[0].titulo, "Plano de la casa");
    assert_eq!(pedido_de(&v[0])["que"], json!({ "tipo": "hoja", "proyecto": "P1", "referencia": "dib-1" }));
    let v = buscar(&r, "hoja suelta");
    assert_eq!(pedido_de(&v[0])["que"]["referencia"], "dib-2");
    let v = buscar(&r, "pagina uno");
    assert_eq!(v[0].titulo, "Página uno");
    assert!(v[0].subtitulo.starts_with("Hoja"));
}

#[test]
fn notas_del_chat_y_md_sueltas() {
    let r = raiz("notas");
    let v = buscar(&r, "idea");
    assert_eq!(v[0].titulo, "Idea brillante");
    assert_eq!(pedido_de(&v[0])["que"], json!({ "tipo": "nota", "proyecto": null, "codigo": "n1" }));
    // Por el texto de dentro tambien, pero por debajo.
    let v = buscar(&r, "integral");
    assert!(v.iter().any(|x| x.titulo == "Idea brillante"));
    let v = buscar(&r, "apuntes");
    assert_eq!(pedido_de(&v[0])["que"]["tipo"], "fichero");
}

#[test]
fn audios_por_nombre_sin_tildes_y_por_transcripcion() {
    let r = raiz("audios");
    let v = buscar(&r, "calculo");
    assert_eq!(v[0].titulo, "Clase de cálculo");
    assert!(v[0].subtitulo.starts_with("Audio · Mensajes guardados"));
    assert_eq!(pedido_de(&v[0])["que"]["tipo"], "fichero");
    let v = buscar(&r, "integrales dobles");
    assert_eq!(v[0].titulo, "Clase de cálculo");
    let v = buscar(&r, "sin nombre");
    assert!(v[0].titulo.starts_with("Nota de voz sin nombre"));
}

#[test]
fn lo_del_buzon_y_los_proyectos_enlazados_no_salen() {
    let r = raiz("buzon");
    assert!(buscar(&r, "buzon").is_empty());
    assert!(!buscar(&r, "otro").iter().any(|x| x.titulo == "otro"));
    assert!(!buscar(&r, "vasos").iter().any(|x| x.titulo == "Vasos"));
}

#[test]
fn vacio_da_las_funciones_y_los_proyectos_recientes() {
    let r = raiz("vacio");
    let v = buscar(&r, "");
    let titulos: Vec<&str> = v.iter().map(|x| x.titulo.as_str()).collect();
    assert_eq!(
        titulos,
        ["Chat", "Tareas", "Lienzo nuevo", "Nota nueva", "Grabar audio", "Añadir a proyecto", "Abrir PixPin", "Gestión de proyectos", "Thesis", "Mensajes guardados", "Chat de clase"]
    );
    assert_eq!(v[1].accion, Accion::Consulta("pp tareas ".into()));
    assert_eq!(pedido_de(&v[2])["accion"], "lienzo_nuevo");
    assert!(v[4].subtitulo.contains("micrófono flotante"), "{}", v[4].subtitulo);
    // Un proyecto ya no abre la app: entra en su chat.
    assert_eq!(v[7].accion, Accion::Consulta("pp Gestión de proyectos > ".into()));
    assert_eq!(v[7].autocompletar.as_deref(), Some("pp Gestión de proyectos > "));
    assert_eq!(v[9].accion, Accion::Consulta("pp Mensajes guardados > ".into()));
}

#[test]
fn una_funcion_por_su_nombre_o_un_alias_sale_la_primera() {
    let r = raiz("funciones");
    assert_eq!(buscar(&r, "tar")[0].titulo, "Tareas");
    assert_eq!(buscar(&r, "canv")[0].titulo, "Lienzo nuevo");
    assert_eq!(buscar(&r, "gest")[0].titulo, "Gestión de proyectos");
    assert!(buscar(&r, "x").len() <= crate::resultados::MAXIMO);
}

#[test]
fn verbos_con_texto_y_proyecto() {
    let r = raiz("verbos");
    let v = buscar(&r, "chat hola que tal @gest");
    assert_eq!(v[0].titulo, "Escribir en el chat: hola que tal");
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "chat", "texto": "hola que tal", "proyecto": "P1" }));
    // `@gest` no es el nombre entero: se sugieren proyectos para completarlo.
    assert!(v.iter().any(|x| x.accion == Accion::Consulta("pp chat hola que tal @Gestión de proyectos".into())));
    let v = buscar(&r, "chat hola @Gestión de proyectos");
    assert_eq!(v.len(), 1, "con el nombre entero ya no se sugiere nada");

    let v = buscar(&r, "nota idea feliz");
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "nota_nueva", "texto": "idea feliz", "proyecto": null }));
    let v = buscar(&r, "canvas plano");
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "lienzo_nuevo", "nombre": "plano", "proyecto": null }));
    assert_eq!(v[1].titulo, "Plano de la casa", "debajo, los lienzos que ya se llaman asi");
    let v = buscar(&r, "grabar Clase 4 @thesis");
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "grabar", "nombre": "Clase 4", "proyecto": "P2" }));
    let v = buscar(&r, "grabar");
    assert_eq!(v[0].accion, Accion::Consulta("pp grabar ".into()));
    let v = buscar(&r, "chat @nadaparecido");
    assert!(v[0].subtitulo.contains("no hay ningún proyecto"));
}

#[test]
fn las_listas_de_tareas_y_una_nueva() {
    let r = raiz("listas");
    let v = buscar(&r, "tareas");
    assert_eq!(v.len(), 2);
    // La mas reciente primero; las dos se llaman igual y se distinguen por proyecto.
    assert_eq!(v[0].accion, Accion::Consulta("pp tareas Compra · Gestión de proyectos > ".into()));
    assert_eq!(v[1].accion, Accion::Consulta("pp tareas Compra · Mensajes guardados > ".into()));
    assert!(v[1].subtitulo.contains("2 pendientes de 3"), "{}", v[1].subtitulo);
    let v = buscar(&r, "todo Viaje");
    assert_eq!(v[0].titulo, "Nueva lista: Viaje");
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "lista_nueva", "titulo": "Viaje", "proyecto": null }));
    let v = buscar(&r, "tareas compra");
    assert!(!v.iter().any(|x| x.titulo.starts_with("Nueva lista")), "ya existe una que se llama asi");
}

#[test]
fn dentro_de_una_lista_marcar_y_anadir() {
    let r = raiz("lista");
    let v = buscar(&r, "tareas Compra · Mensajes guardados > ");
    let titulos: Vec<&str> = v.iter().map(|x| x.titulo.as_str()).collect();
    assert_eq!(titulos, ["☐ pan", "☐ huevos", "☑ leche"]);
    assert_eq!(
        *pedido_de(&v[1]),
        json!({ "pixpin": 1, "accion": "marcar_tarea", "proyecto": null, "codigo": "t1", "indice": 2, "hecha": true })
    );
    assert_eq!(pedido_de(&v[2])["hecha"], false);
    let Accion::PedirYSeguir { consulta, .. } = &v[0].accion else { panic!() };
    assert_eq!(consulta, "pp tareas Compra · Mensajes guardados > ");

    let v = buscar(&r, "tareas Compra · Mensajes guardados > yogur");
    assert_eq!(v[0].titulo, "Añadir tarea: yogur");
    assert_eq!(
        v[0].accion,
        Accion::PedirYSeguir {
            pedido: json!({ "pixpin": 1, "accion": "anadir_tarea", "texto": "yogur", "proyecto": null, "codigo": "t1" }),
            consulta: "pp tareas Compra · Mensajes guardados > ".into(),
        }
    );
    // Lo que ya existe no se ofrece anadir, y el filtro deja solo esa.
    let v = buscar(&r, "tareas compra · mensajes guardados > Pan");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].titulo, "☐ pan");
    // Con el titulo solo vale tambien (la de Gestion, que es la mas nueva).
    let v = buscar(&r, "tareas compra > ");
    assert_eq!(v[0].titulo, "☐ tornillos");
    assert_eq!(buscar(&r, "tareas nada de nada > x")[0].accion, Accion::Consulta("pp tareas ".into()));
}

// --- El dialogo con Flow ---------------------------------------------------

#[derive(Default)]
struct DeMentira {
    pedidos: Vec<Value>,
    /// Los que no arrancan la app (`iconos`).
    avisos: Vec<Value>,
    /// Lo abierto con el programa de Windows.
    lanzados: Vec<String>,
    falla: bool,
}

impl Mensajero for DeMentira {
    fn enviar(&mut self, json: &str) -> Envio {
        self.pedidos.push(serde_json::from_str(json).unwrap());
        if self.falla { Envio::SinApp("prueba".into()) } else { Envio::Aceptado }
    }
    fn avisar(&mut self, json: &str) -> Envio {
        self.avisos.push(serde_json::from_str(json).unwrap());
        Envio::Aceptado
    }
    fn abrir_con_windows(&mut self, ruta: &str) -> bool {
        self.lanzados.push(ruta.to_string());
        true
    }
}

fn plugin(r: &Path) -> Plugin<DeMentira, Vec<u8>> {
    let mut p = Plugin::nuevo(Datos::nuevo(r.to_path_buf()), DeMentira::default(), Vec::new(), "icono.png".into());
    p.espera_lista = std::time::Duration::ZERO;
    p
}

/// Las lineas que el plugin escribio, como JSON.
fn salida(p: &Plugin<DeMentira, Vec<u8>>) -> Vec<Value> {
    String::from_utf8(p.salida().clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("cada linea es un JSON"))
        .collect()
}

fn consulta(id: u64, search: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": "query",
        "params": [{ "rawQuery": format!("pp {search}"), "search": search, "actionKeyword": "pp", "isReQuery": false }, {}] })
    .to_string()
}

#[test]
fn initialize_y_query_con_la_forma_de_flow() {
    let r = raiz("rpc-query");
    let ajustes = r.join("ajustes");
    let mut p = plugin(&r);
    let entrada = format!(
        "{}\r\n{}\n",
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": [{ "currentPluginMetadata": { "pluginDirectory": r.to_string_lossy(), "pluginSettingsDirectoryPath": ajustes.to_string_lossy() } }] }),
        consulta(2, "objetivos")
    );
    bucle(&mut p, entrada.as_bytes());
    let s = salida(&p);
    assert_eq!(s.len(), 2);
    assert_eq!(s[0], json!({ "jsonrpc": "2.0", "id": 1, "result": { "hide": false } }));
    assert_eq!(s[1]["id"], 2);
    let res = &s[1]["result"];
    assert_eq!(res["settingsChange"], json!({}));
    assert_eq!(res["debugMessage"], "");
    let primero = &res["result"][0];
    assert_eq!(primero["title"], "Objetivos e Indicadores.md");
    // Su icono es el de su extension, que la app ya pinto: sin glifo.
    assert!(primero["icoPath"].as_str().unwrap().ends_with(r"iconos-de-extension\md.png"), "{}", primero["icoPath"]);
    assert!(primero.get("glyph").is_none());
    assert_eq!(primero["jsonRPCAction"]["method"], "pedir");
    assert_eq!(primero["jsonRPCAction"]["dontHideAfterAction"], false);
    assert!(primero["subTitle"].is_string() && primero["score"].is_i64() && primero["contextData"].is_object());
    // El registro va a un fichero de la carpeta de ajustes.
    assert!(ajustes.join("pixpin-lanzador.log").is_file());
}

#[test]
fn una_accion_llega_envuelta_y_manda_el_pedido() {
    let r = raiz("rpc-pedir");
    let mut p = plugin(&r);
    let pedido = json!({ "pixpin": 1, "accion": "abrir", "que": { "tipo": "proyecto", "proyecto": "P1" } });
    p.atender(&json!({ "jsonrpc": "2.0", "id": 7, "method": "pedir", "params": [[pedido]] }).to_string());
    assert_eq!(p.mensajero.pedidos, vec![pedido]);
    assert_eq!(salida(&p), vec![json!({ "jsonrpc": "2.0", "id": 7, "result": { "hide": true } })]);
}

#[test]
fn si_la_app_no_esta_se_avisa_con_showmsg() {
    let r = raiz("rpc-falla");
    let mut p = plugin(&r);
    p.mensajero.falla = true;
    p.atender(&json!({ "jsonrpc": "2.0", "id": 7, "method": "pedir", "params": [[{ "pixpin": 1, "accion": "ventana_principal" }]] }).to_string());
    let s = salida(&p);
    assert_eq!(s[0]["result"]["hide"], true);
    assert_eq!(s[1]["method"], "ShowMsg");
    assert!(s[1]["params"][1].as_str().unwrap().contains("No se pudo abrir PixPin"));
}

#[test]
fn marcar_una_tarea_la_ensena_marcada_sin_cerrar_flow() {
    let r = raiz("rpc-marcar");
    let mut p = plugin(&r);
    let q = "tareas Compra · Mensajes guardados > ";
    p.atender(&consulta(1, q));
    let s = salida(&p);
    let pan = &s[0]["result"]["result"][0];
    assert_eq!(pan["title"], "☐ pan");
    assert_eq!(pan["jsonRPCAction"]["dontHideAfterAction"], true);
    // Flow manda la accion con sus parametros envueltos.
    let accion = json!({ "jsonrpc": "2.0", "id": 2, "method": pan["jsonRPCAction"]["method"],
        "params": [pan["jsonRPCAction"]["parameters"]] });
    p.atender(&accion.to_string());
    assert_eq!(p.mensajero.pedidos[0]["accion"], "marcar_tarea");
    let s = salida(&p);
    assert_eq!(s[1]["method"], "ChangeQuery");
    assert_eq!(s[1]["params"], json!([format!("pp {q}"), true]));
    assert_eq!(s[2], json!({ "jsonrpc": "2.0", "id": 2, "result": { "hide": false } }));
    // Flow contesta al ChangeQuery (se ignora) y vuelve a consultar.
    p.atender(&json!({ "jsonrpc": "2.0", "id": s[1]["id"], "result": null }).to_string());
    p.atender(&consulta(3, q));
    let s = salida(&p);
    let titulos: Vec<&str> = s[3]["result"]["result"].as_array().unwrap().iter().map(|r| r["title"].as_str().unwrap()).collect();
    assert_eq!(titulos, ["☐ huevos", "☑ pan", "☑ leche"]);
}

#[test]
fn anadir_una_tarea_la_ensena_en_la_lista() {
    let r = raiz("rpc-anadir");
    let mut p = plugin(&r);
    p.atender(&consulta(1, "tareas Compra · Mensajes guardados > yogur"));
    let anadir = salida(&p)[0]["result"]["result"][0]["jsonRPCAction"].clone();
    p.atender(&json!({ "jsonrpc": "2.0", "id": 2, "method": anadir["method"], "params": [anadir["parameters"]] }).to_string());
    assert_eq!(p.mensajero.pedidos[0]["accion"], "anadir_tarea");
    p.atender(&consulta(3, "tareas Compra · Mensajes guardados > "));
    let s = salida(&p);
    let ultima = s.last().unwrap()["result"]["result"].as_array().unwrap().iter().any(|r| r["title"] == "☐ yogur");
    assert!(ultima);
}

#[test]
fn lo_que_no_es_una_peticion_no_se_contesta() {
    let r = raiz("rpc-ruido");
    let mut p = plugin(&r);
    for l in [
        r#"{"jsonrpc":"2.0","method":"$/cancelRequest","params":{"id":3}}"#,
        r#"{"jsonrpc":"2.0","id":1000000,"result":null}"#,
        "esto no es json",
        "",
        r#"{"jsonrpc":"2.0","method":"query","params":[{"search":"x"}]}"#,
    ] {
        p.atender(l);
    }
    assert!(salida(&p).is_empty());
    p.atender(r#"{"jsonrpc":"2.0","id":9,"method":"inventado","params":[]}"#);
    assert_eq!(salida(&p)[0]["error"]["code"], -32601);
}

#[test]
fn menu_contextual_y_cerrar() {
    let r = raiz("rpc-menu");
    let mut p = plugin(&r);
    p.atender(r#"{"jsonrpc":"2.0","id":1,"method":"context_menu","params":[null]}"#);
    assert_eq!(salida(&p)[0]["result"]["result"], json!([]));
    p.atender(&consulta(2, "calculo"));
    let ctx = salida(&p)[1]["result"]["result"][0]["contextData"].clone();
    p.atender(&json!({ "jsonrpc": "2.0", "id": 3, "method": "context_menu", "params": [ctx] }).to_string());
    let s = salida(&p);
    let menu = s[2]["result"]["result"].as_array().unwrap();
    let titulos: Vec<&str> = menu.iter().map(|r| r["title"].as_str().unwrap()).collect();
    assert_eq!(
        titulos,
        ["Abrir", "Ver en el chat", "Abrir con el programa de Windows", "Mostrar en la carpeta", "Copiar texto", "Copiar ruta"]
    );
    assert_eq!(
        menu[1]["jsonRPCAction"]["parameters"][0]["que"],
        json!({ "tipo": "mensaje", "proyecto": null, "codigo": "v1" })
    );
    let entrada = "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"close\"}\n".to_string() + &consulta(5, "x");
    bucle(&mut p, entrada.as_bytes());
    assert!(p.terminado);
    assert_eq!(salida(&p).len(), 4, "tras close no se atiende nada mas");
}

#[test]
fn una_lista_nueva_espera_y_entra_en_ella() {
    let r = raiz("rpc-nueva");
    let mut p = plugin(&r);
    p.atender(&consulta(1, "tareas Viaje"));
    let a = salida(&p)[0]["result"]["result"][0]["jsonRPCAction"].clone();
    p.atender(&json!({ "jsonrpc": "2.0", "id": 2, "method": a["method"], "params": [a["parameters"]] }).to_string());
    assert_eq!(p.mensajero.pedidos[0]["accion"], "lista_nueva");
    let s = salida(&p);
    assert_eq!(s[1]["params"], json!(["pp tareas Viaje > ", true]));
}

// --- El chat de un proyecto, los iconos y el menu ----------------------------

/// Como `buscar`, pero mirando los iconos de la carpeta de la prueba.
fn buscar_con_iconos(r: &Path, q: &str) -> (Vec<Resultado>, Vec<String>) {
    let mut d = Datos::nuevo(r.to_path_buf());
    let mut c = ctx();
    c.iconos = Some(crate::iconos::carpeta(r));
    let v = resultados(&d.proyectos(), q, &c);
    let faltan = c.extensiones_que_faltan();
    (v, faltan)
}

fn titulos(v: &[Resultado]) -> Vec<&str> {
    v.iter().map(|x| x.titulo.as_str()).collect()
}

/// El menu de un resultado.
fn menu_de(r: &Resultado) -> Vec<Resultado> {
    crate::resultados::menu(r.contexto.as_ref().expect("lleva contextData"), &ctx())
}

#[test]
fn el_chat_de_un_proyecto_de_lo_ultimo_a_lo_primero_con_todas_las_clases() {
    let r = raiz("chat");
    let (mut v, faltan) = buscar_con_iconos(&r, "Gestión de proyectos > ");
    // La segunda fila fija: el recuadro para soltar archivos en este proyecto.
    let arrastrar = v.remove(1);
    assert_eq!(arrastrar.titulo, "Añadir arrastrando (recuadro flotante)");
    assert_eq!(*pedido_de(&arrastrar), json!({ "pixpin": 1, "accion": "soltar", "proyecto": "P1" }));
    assert_eq!(
        titulos(&v),
        [
            "Abrir proyecto en la app",
            "croquis.png",
            "hola equipo, mañana a las 9",
            "Acta",
            "Vasos",
            "Compra",
            "otro",
            "Remoto.pdf",
            "Objetivos e Indicadores.md",
            "Página uno",
            "Plano de la casa",
            "revisar el presupuesto",
        ]
    );
    assert_eq!(*pedido_de(&v[0]), json!({ "pixpin": 1, "accion": "abrir", "que": { "tipo": "proyecto", "proyecto": "P1" } }));
    // Foto: su miniatura, sin glifo.
    let foto = r.join("proyectos").join("P1").join("archivos").join("croquis.png");
    assert_eq!(v[1].icono.as_deref(), Some(foto.to_string_lossy().as_ref()));
    assert!(v[1].subtitulo.starts_with("Foto · "), "{}", v[1].subtitulo);
    // Un texto: su primera linea, «Mensaje · <hora>», y abre el mensaje.
    assert!(v[2].subtitulo.starts_with("Mensaje · "), "{}", v[2].subtitulo);
    assert_eq!(pedido_de(&v[2])["que"], json!({ "tipo": "mensaje", "proyecto": "P1", "codigo": "x1" }));
    // Una nota con su .md: al editor de notas.
    assert_eq!(pedido_de(&v[3])["que"], json!({ "tipo": "nota", "proyecto": "P1", "codigo": "n2" }));
    // Otra mini-app: su mensaje; una lista: entra en ella.
    assert_eq!(pedido_de(&v[4])["que"]["codigo"], "m3");
    assert_eq!(v[5].accion, Accion::Consulta("pp tareas Compra · Gestión de proyectos > ".into()));
    assert!(v[5].subtitulo.starts_with("Lista de tareas · 1 pendiente de 1"), "{}", v[5].subtitulo);
    // El acceso directo a otro proyecto entra en su chat.
    assert_eq!(v[6].accion, Accion::Consulta("pp Thesis > ".into()));
    // Archivos: el icono de su extension si ya esta; si no, glifo y se pide.
    assert_eq!(v[7].icono, None);
    assert!(v[8].icono.as_deref().unwrap().ends_with(r"iconos-de-extension\md.png"));
    assert_eq!(faltan, ["pdf"]);
    // Lienzos y hojas al lienzo; el audio con lo que dura.
    assert_eq!(pedido_de(&v[10])["que"], json!({ "tipo": "hoja", "proyecto": "P1", "referencia": "dib-1" }));
    assert!(v[11].subtitulo.starts_with("Audio · 1:15 · "), "{}", v[11].subtitulo);
    assert!(v[11].icono.is_none(), "los audios, su glifo");
    // Lo del buzon no sale.
    assert!(!titulos(&v).contains(&"del buzon"));
}

#[test]
fn en_el_chat_se_filtra_y_abrir_el_proyecto_baja_al_final() {
    let r = raiz("chat-filtro");
    let v = buscar(&r, "gestion de proyectos > presupuesto");
    assert_eq!(titulos(&v), ["revisar el presupuesto", "Abrir proyecto en la app", "Añadir arrastrando (recuadro flotante)"]);
    let v = buscar(&r, "Gestión de proyectos > nada de esto");
    assert_eq!(titulos(&v), ["Nada con «nada de esto» en «Gestión de proyectos»", "Abrir proyecto en la app", "Añadir arrastrando (recuadro flotante)"]);
    assert_eq!(v[0].accion, Accion::Consulta("pp Gestión de proyectos > ".into()));
    // Mensajes guardados por su nombre de siempre, y un trozo del nombre vale.
    let v = buscar(&r, "Mensajes guardados > ");
    assert_eq!(v[2].titulo, "Compra", "lo mas nuevo de guardados es la lista");
    assert_eq!(*pedido_de(&v[1]), json!({ "pixpin": 1, "accion": "soltar", "proyecto": null }));
    assert_eq!(buscar(&r, "thes > ")[0].titulo, "Abrir proyecto en la app");
    assert!(buscar(&r, "thes > ")[0].subtitulo.starts_with("Thesis"));
    assert!(buscar(&r, "zzz > ")[0].titulo.starts_with("No encuentro el proyecto"));
}

#[test]
fn un_proyecto_que_empieza_por_un_verbo_y_no_choca_con_las_tareas() {
    let r = raiz("chat-verbo");
    let v = buscar(&r, "Chat de clase > ");
    assert_eq!(titulos(&v), ["Abrir proyecto en la app", "Añadir arrastrando (recuadro flotante)", "El chat está vacío"]);
    // «chat hola > adios» sigue siendo escribir en el chat.
    assert_eq!(buscar(&r, "chat hola > adios")[0].titulo, "Escribir en el chat: hola > adios");
    // y «tareas X > » sigue siendo la lista.
    assert_eq!(buscar(&r, "tareas compra > ")[0].titulo, "☐ tornillos");
}

#[test]
fn el_menu_de_cada_tipo() {
    let r = raiz("menus");
    let (mut v, _) = buscar_con_iconos(&r, "Gestión de proyectos > ");
    // El recuadro lleva el menu del proyecto.
    assert_eq!(menu_de(&v[1])[0].titulo, "Abrir en la app");
    v.remove(1);
    let t = |i: usize| menu_de(&v[i]).into_iter().map(|x| x.titulo).collect::<Vec<_>>();
    // El proyecto.
    assert_eq!(
        t(0),
        ["Abrir en la app", "Ver contenido", "Añadir arrastrando (recuadro flotante)", "Lienzo nuevo aquí", "Nota nueva aquí", "Abrir la carpeta"]
    );
    assert_eq!(*pedido_de(&menu_de(&v[0])[2]), json!({ "pixpin": 1, "accion": "soltar", "proyecto": "P1" }));
    // La foto.
    assert_eq!(
        t(1),
        ["Abrir", "Sacar a la pantalla (pin)", "Ver en el chat", "Abrir con el programa de Windows", "Mostrar en la carpeta", "Copiar ruta"]
    );
    let m = menu_de(&v[1]);
    let carpeta = r.join("proyectos").join("P1").join("archivos");
    let foto = carpeta.join("croquis.png").to_string_lossy().to_string();
    assert_eq!(m[0].accion, v[1].accion, "Abrir es lo mismo que Intro");
    assert_eq!(*pedido_de(&m[1]), json!({ "pixpin": 1, "accion": "pinear", "ruta": foto }));
    assert_eq!(m[3].accion, Accion::Windows(foto.clone()));
    assert_eq!(m[4].accion, Accion::Carpeta { carpeta: carpeta.to_string_lossy().to_string(), fichero: foto.clone() });
    assert_eq!(m[5].accion, Accion::Copiar(foto));
    // Un texto: pin por su codigo y copiar el texto entero.
    assert_eq!(t(2), ["Abrir", "Sacar a la pantalla (pin)", "Ver en el chat", "Copiar texto", "Abrir la carpeta del proyecto"]);
    let m = menu_de(&v[2]);
    assert_eq!(*pedido_de(&m[1]), json!({ "pixpin": 1, "accion": "pinear", "proyecto": "P1", "codigo": "x1" }));
    assert_eq!(m[3].accion, Accion::Copiar("hola equipo, mañana a las 9\nsegunda línea".into()));
    // Una lista: Abrir entra en ella.
    assert_eq!(menu_de(&v[5])[0].accion, v[5].accion);
    // Un archivo que no esta en este equipo: nada de Windows ni de pin.
    assert_eq!(t(7), ["Abrir", "Ver en el chat", "Abrir la carpeta del proyecto"]);
    // El lienzo.
    assert_eq!(t(10), ["Abrir", "Sacar a la pantalla (pin)", "Ver en el chat", "Abrir la carpeta del proyecto"]);
    // El audio: sin pin, con su transcripcion.
    assert_eq!(
        t(11),
        ["Abrir", "Ver en el chat", "Abrir con el programa de Windows", "Mostrar en la carpeta", "Copiar texto", "Copiar ruta"]
    );
}

/// Una peticion del modo v1, en un plugin nuevo (cada una es un proceso):
/// la respuesta, los avisos que dejo pendientes, y lo que mando.
fn v1(r: &Path, peticion: Value) -> (Value, Vec<Value>, DeMentira) {
    let mut p = plugin(r);
    p.diferir_avisos = true;
    let respuesta = crate::rpc::una_vez(&mut p, &peticion);
    let avisos = p.avisos().to_vec();
    p.mandar_avisos();
    (respuesta, avisos, p.mensajero)
}

#[test]
fn modo_v1_consulta_menu_y_acciones() {
    let r = raiz("v1");
    let (q, avisos, m) = v1(&r, json!({ "method": "query", "parameters": ["Gestión de proyectos > "] }));
    let lista = q["result"].as_array().unwrap();
    assert_eq!(lista[0]["title"], "Abrir proyecto en la app");
    assert_eq!(lista[0]["autoCompleteText"], "p Gestión de proyectos > ", "en v1, la palabra clave «p»");
    // Los iconos que faltan se piden despues de contestar, y sin arrancar la app.
    assert_eq!(avisos, vec![json!({ "pixpin": 1, "accion": "iconos", "extensiones": ["pdf"] })]);
    assert_eq!(m.avisos.len(), 1);
    assert!(m.pedidos.is_empty());
    // El menu de la foto.
    let (menu, _, _) = v1(&r, json!({ "method": "context_menu", "parameters": [lista[2]["contextData"]] }));
    let menu = menu["result"].as_array().unwrap().clone();
    assert_eq!(menu[1]["title"], "Sacar a la pantalla (pin)");
    // Cada accion del menu, como la manda Flow v1: sus parametros sin envolver.
    let accion = |i: usize| {
        let a = &menu[i]["jsonRPCAction"];
        json!({ "method": a["method"], "parameters": a["parameters"] })
    };
    let (res, _, m) = v1(&r, accion(1));
    assert_eq!(res, json!({}));
    assert_eq!(m.pedidos[0]["accion"], "pinear");
    let (res, _, m) = v1(&r, accion(3));
    assert_eq!(res, json!({}));
    assert!(m.lanzados[0].ends_with("croquis.png"));
    let (carpeta, _, _) = v1(&r, accion(4));
    assert_eq!(carpeta["method"], "Flow.Launcher.OpenDirectory");
    assert!(carpeta["parameters"][1].as_str().unwrap().ends_with("croquis.png"));
    let (copiar, _, _) = v1(&r, accion(5));
    assert_eq!(copiar["method"], "Flow.Launcher.CopyToClipboard");
    // Intro en un proyecto de la busqueda: entra en su chat.
    let (q, _, _) = v1(&r, json!({ "method": "query", "parameters": ["thesis"] }));
    let a = &q["result"][0]["jsonRPCAction"];
    assert_eq!(a["method"], "consulta");
    let (cambio, _, _) = v1(&r, json!({ "method": a["method"], "parameters": a["parameters"] }));
    assert_eq!(cambio, json!({ "method": "Flow.Launcher.ChangeQuery", "parameters": ["p Thesis > ", true] }));
}

#[test]
fn en_v2_los_iconos_que_faltan_se_piden_tras_contestar() {
    let r = raiz("v2-iconos");
    let mut p = plugin(&r);
    p.atender(&consulta(1, "remoto"));
    assert_eq!(salida(&p).len(), 1, "la respuesta, y nada mas por stdout");
    assert_eq!(p.mensajero.avisos, vec![json!({ "pixpin": 1, "accion": "iconos", "extensiones": ["pdf"] })]);
    assert!(p.mensajero.pedidos.is_empty());
}

#[test]
fn anadir_a_proyecto_con_el_recuadro_flotante() {
    let r = raiz("soltar");
    let soltar = |p: Value| json!({ "pixpin": 1, "accion": "soltar", "proyecto": p });
    // Por su nombre o un alias, la funcion sale la primera.
    for q in ["solt", "adjun", "agreg", "dro"] {
        assert_eq!(buscar(&r, q)[0].titulo, "Añadir a proyecto", "{q}");
    }
    assert_eq!(buscar(&r, "añad")[0].accion, Accion::Consulta("pp añadir ".into()));
    // Sin nombre: guardados primero y despues los proyectos, del mas tocado.
    let v = buscar(&r, "añadir");
    assert_eq!(
        titulos(&v),
        ["Añadir a «Mensajes guardados»", "Añadir a «Gestión de proyectos»", "Añadir a «Thesis»", "Añadir a «Chat de clase»"]
    );
    assert_eq!(*pedido_de(&v[0]), soltar(Value::Null));
    assert_eq!(*pedido_de(&v[1]), soltar(json!("P1")));
    // Con texto, filtra; con alias o tildes, igual.
    let v = buscar(&r, "anadir thes");
    assert_eq!(titulos(&v), ["Añadir a «Thesis»"]);
    assert_eq!(*pedido_de(&v[0]), soltar(json!("P2")));
    assert_eq!(titulos(&buscar(&r, "subir gestion")), ["Añadir a «Gestión de proyectos»"]);
    assert_eq!(titulos(&buscar(&r, "agregar mensajes")), ["Añadir a «Mensajes guardados»"]);
    // Con @ tambien.
    assert_eq!(*pedido_de(&buscar(&r, "adjuntar @Thesis")[0]), soltar(json!("P2")));
    // Nada que encaje: a guardados, diciendolo.
    let v = buscar(&r, "añadir zzz");
    assert_eq!(v.len(), 1);
    assert!(v[0].titulo.contains("No hay ningún proyecto «zzz»"));
    assert_eq!(*pedido_de(&v[0]), soltar(Value::Null));
}
