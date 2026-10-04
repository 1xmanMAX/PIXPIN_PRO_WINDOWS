//! El menu contextual de un resultado: flecha derecha o Mayus+Intro en Flow,
//! que llama a `context_menu` con su `contextData`.
//!
//! El `contextData` lo pone cada resultado ([`Resultado::con_menu`]) con lo
//! que hace falta, y aqui solo se ofrece lo que aplica:
//!
//! - `abrir`: lo que hace Intro (`{metodo, parametros}`) → «Abrir»;
//! - `pin`: el pedido `pinear` → «Sacar a la pantalla (pin)»;
//! - `codigo` (y `proyecto`) → «Ver en el chat»;
//! - `ruta` (fichero de este equipo) → «Abrir con el programa de Windows»,
//!   «Mostrar en la carpeta», «Copiar ruta»;
//! - `texto` → «Copiar texto»;
//! - `carpeta` (la del proyecto) → «Abrir la carpeta del proyecto».
//!
//! Un proyecto (`tipo: "proyecto"`): «Abrir en la app», «Ver contenido»,
//! «Añadir arrastrando» (pedido `soltar`)
//! (`consulta`), lienzo y nota nuevos ahi, y su carpeta.
//!
//! Una tarea lleva ademas `mover` (`{proyecto, codigo, indice, destinos}`):
//! «Mover a «lista»» por cada otra lista (pedido `mover_tarea`).
//!
//! Una leccion (`tipo: "leccion"`, `ruta` de su `.leccion`, `titulo`, `texto`,
//! `pin` de su primera foto): «Abrir la ficha», «Ver en la lista de
//! lecciones», «Copiar texto», su foto como pin y «Mostrar en la carpeta».
//!
//! Una captura (`tipo: "captura"`, `ruta`, `conservada`): «Conservar»,
//! «Copiar imagen», «Borrar», «Mostrar en la carpeta», «Abrir galería».

use crate::resultados::{glifo, pedido, pedido_ventana, Accion, Contexto, Resultado};
use serde_json::{json, Value};

fn muestra(texto: &str) -> String {
    let t: String = texto.chars().take(80).collect();
    t.replace(['\r', '\n'], " ")
}

pub fn menu(contexto: &Value, ctx: &Contexto) -> Vec<Resultado> {
    let _ = ctx;
    let texto_de = |k: &str| contexto.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let proyecto = contexto.get("proyecto").cloned().unwrap_or(Value::Null);
    let tipo = texto_de("tipo").unwrap_or("");
    let mut v = Vec::new();

    if tipo == "proyecto" {
        v.push(Resultado::nuevo(
            "Abrir en la app",
            "Su chat en PixPin",
            glifo::PIXPIN,
            Accion::Pedido(pedido("abrir", json!({ "que": { "tipo": "proyecto", "proyecto": proyecto } }))),
        ));
        if let Some(q) = texto_de("consulta") {
            v.push(Resultado::nuevo("Ver contenido", "Su chat aquí, de lo último a lo primero", glifo::CHAT, Accion::Consulta(q.to_string())));
        }
        v.push(Resultado::nuevo(
            "Añadir arrastrando (recuadro flotante)",
            "Suelta archivos en el recuadro y van a su chat",
            glifo::ADJUNTAR,
            Accion::Pedido(pedido("soltar", json!({ "proyecto": proyecto }))),
        ));
        v.push(Resultado::nuevo(
            "Lienzo nuevo aquí",
            "Un lienzo en blanco en este proyecto",
            glifo::LIENZO,
            Accion::Pedido(pedido("lienzo_nuevo", json!({ "proyecto": proyecto }))),
        ));
        v.push(Resultado::nuevo(
            "Nota nueva aquí",
            "Una nota en este proyecto",
            glifo::NOTA,
            Accion::Pedido(pedido("nota_nueva", json!({ "proyecto": proyecto }))),
        ));
        if let Some(c) = texto_de("carpeta") {
            v.push(Resultado::nuevo("Abrir la carpeta", c, glifo::CARPETA, Accion::Carpeta { carpeta: c.into(), fichero: String::new() }));
        }
        return v;
    }

    if tipo == "captura" {
        let Some(r) = texto_de("ruta") else { return v };
        let carpeta = std::path::Path::new(r).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        if !contexto.get("conservada").and_then(Value::as_bool).unwrap_or(false) {
            v.push(Resultado::nuevo(
                "Conservar",
                "No se borra a la semana: se guarda en el chat",
                glifo::CONSERVAR,
                Accion::Pedido(pedido("conservar_captura", json!({ "ruta": r }))),
            ));
        }
        v.push(Resultado::nuevo(
            "Copiar imagen",
            "Al portapapeles, para pegarla",
            glifo::COPIAR,
            Accion::Pedido(pedido("copiar_imagen", json!({ "ruta": r }))),
        ));
        v.push(Resultado::nuevo(
            "Borrar",
            "A la papelera",
            glifo::BORRAR,
            Accion::Pedido(pedido("borrar_captura", json!({ "ruta": r }))),
        ));
        v.push(Resultado::nuevo("Mostrar en la carpeta", &carpeta, glifo::CARPETA, Accion::Carpeta { carpeta: carpeta.clone(), fichero: r.into() }));
        v.push(Resultado::nuevo(
            "Abrir galería",
            "Todas las capturas",
            glifo::GALERIA,
            Accion::Pedido(pedido_ventana("galeria")),
        ));
        return v;
    }

    if tipo == "leccion" {
        if let Some(a) = contexto.get("abrir").and_then(Accion::de_valor) {
            v.push(Resultado::nuevo("Abrir la ficha", "Como Intro: la lección en PixPin", glifo::ABRIR, a));
        }
        let titulo = texto_de("titulo").unwrap_or("");
        v.push(Resultado::nuevo(
            "Ver en la lista de lecciones",
            "La ventana de Lecciones, buscándola",
            glifo::VENTANA,
            Accion::Pedido(pedido("lecciones", json!({ "consulta": titulo, "proyecto": proyecto }))),
        ));
        if let Some(t) = texto_de("texto") {
            v.push(Resultado::nuevo("Copiar texto", muestra(t), glifo::COPIAR, Accion::Copiar(t.into())));
        }
        if let Some(pin) = contexto.get("pin").filter(|p| p.is_object()) {
            v.push(Resultado::nuevo("Sacar su foto a la pantalla (pin)", "Como pin flotante, siempre encima", glifo::PIN, Accion::Pedido(pin.clone())));
        }
        if let Some(r) = texto_de("ruta") {
            let carpeta = std::path::Path::new(r).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            v.push(Resultado::nuevo("Mostrar en la carpeta", &carpeta, glifo::CARPETA, Accion::Carpeta { carpeta: carpeta.clone(), fichero: r.into() }));
        }
        return v;
    }

    let ruta = texto_de("ruta");
    if let Some(a) = contexto.get("abrir").and_then(Accion::de_valor) {
        let sub = if ruta.is_some() { "Con PixPin, como Intro" } else { "Como Intro" };
        v.push(Resultado::nuevo("Abrir", sub, glifo::ABRIR, a));
    }
    if let Some(pin) = contexto.get("pin").filter(|p| p.is_object()) {
        v.push(Resultado::nuevo(
            "Sacar a la pantalla (pin)",
            "Como pin flotante, siempre encima",
            glifo::PIN,
            Accion::Pedido(pin.clone()),
        ));
    }
    if let Some(codigo) = texto_de("codigo") {
        v.push(Resultado::nuevo(
            "Ver en el chat",
            "Abre el chat de PixPin en este mensaje",
            glifo::CHAT,
            Accion::Pedido(pedido("abrir", json!({ "que": { "tipo": "mensaje", "proyecto": proyecto, "codigo": codigo } }))),
        ));
    }
    if let Some(r) = ruta {
        let carpeta = std::path::Path::new(r).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        v.push(Resultado::nuevo("Abrir con el programa de Windows", r, glifo::WINDOWS, Accion::Windows(r.into())));
        v.push(Resultado::nuevo("Mostrar en la carpeta", &carpeta, glifo::CARPETA, Accion::Carpeta { carpeta: carpeta.clone(), fichero: r.into() }));
    }
    if let Some(t) = texto_de("texto") {
        v.push(Resultado::nuevo("Copiar texto", muestra(t), glifo::COPIAR, Accion::Copiar(t.into())));
    }
    if let Some(r) = ruta {
        v.push(Resultado::nuevo("Copiar ruta", r, glifo::COPIAR, Accion::Copiar(r.into())));
    }
    if let Some(m) = contexto.get("mover").filter(|m| m.is_object()) {
        v.extend(mover_a(m));
    }
    if ruta.is_none() {
        if let Some(c) = texto_de("carpeta") {
            v.push(Resultado::nuevo(
                "Abrir la carpeta del proyecto",
                c,
                glifo::CARPETA,
                Accion::Carpeta { carpeta: c.into(), fichero: String::new() },
            ));
        }
    }
    v
}

/// «Mover a «lista»» por cada destino de una tarea (pedido `mover_tarea`).
fn mover_a(m: &Value) -> Vec<Resultado> {
    let (Some(codigo), Some(indice)) = (m.get("codigo").and_then(Value::as_str), m.get("indice").and_then(Value::as_u64)) else {
        return Vec::new();
    };
    let proyecto = m.get("proyecto").cloned().unwrap_or(Value::Null);
    let Some(destinos) = m.get("destinos").and_then(Value::as_array) else {
        return Vec::new();
    };
    destinos
        .iter()
        .filter_map(|d| {
            let titulo = d.get("titulo").and_then(Value::as_str)?;
            let a_codigo = d.get("a_codigo").and_then(Value::as_str)?;
            let donde = d.get("donde").and_then(Value::as_str).unwrap_or("");
            let a_proyecto = d.get("a_proyecto").cloned().unwrap_or(Value::Null);
            Some(Resultado::nuevo(
                format!("Mover a «{titulo}»"),
                format!("Lista de tareas · {donde}"),
                glifo::MOVER,
                Accion::Pedido(pedido(
                    "mover_tarea",
                    json!({ "proyecto": proyecto, "codigo": codigo, "indice": indice, "a_proyecto": a_proyecto, "a_codigo": a_codigo }),
                )),
            ))
        })
        .collect()
}
