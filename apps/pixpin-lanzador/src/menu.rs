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

use crate::resultados::{glifo, pedido, Accion, Contexto, Resultado};
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
