//! Las pruebas de sacar las listas de tareas. Con `PINDESK_MUESTRAS_TAREAS`
//! puesta (una carpeta), las muestras se dejan ahi para mirarlas: la pagina,
//! el Markdown, el texto y las hojas como SVG.

use super::*;

/// El 1-nov-2026 a las 9:12, domingo.
fn ahora() -> Momento {
    Momento {
        dia: fechas::dias_de(2026, 11, 1),
        minuto: 9 * 60 + 12,
    }
}

const COMPRAS: &str = "# Compras <de> la \"obra\" & más\n\n\
- [ ] Llamar al banco ⏫ #casa 💬 no abren hasta el lunes ⏰ 2026-11-02 17:00 ➕ 2026-10-28\n\
- [ ] Comprar yeso y <cinta> de carrocero ![img 01](pixpin:files/a/yeso.png) 🔼 #obra ➕ 2026-10-30\n\
- [ ] Pedir presupuesto del suelo radiante ⏰ 2026-11-06 09:00\n\
- [ ] Renovar el seguro del coche 🔼 ⏰ 2026-10-30 10:00\n\
- [x] Cemento ➕ 2026-10-20\n\
- [x] Arena de río #obra\n";

const CASA: &str = "# Casa\n\n\
- [ ] Regar las plantas #casa\n\
- [ ] Cambiar la bombilla del pasillo ⏰ 2026-11-01 20:30\n\
- [x] Limpiar los cristales\n";

const HECHA: &str = "# Viaje\n\n- [x] Maletas\n- [x] Billetes ⏰ 2026-10-01 08:00\n";

fn listas() -> Vec<ListaParaExportar> {
    vec![
        ListaParaExportar::del_documento(COMPRAS, "x"),
        ListaParaExportar::del_documento(CASA, "x"),
        ListaParaExportar::del_documento(HECHA, "x"),
    ]
}

/// Una foto de mentira: un PNG de 1×1 como `data:`.
fn foto(enlace: &str) -> Option<String> {
    enlace
        .ends_with("yeso.png")
        .then(|| "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".to_string())
}

#[test]
fn cada_tarea_se_entiende_con_sus_marcas_y_sin_ellas_en_el_titulo() {
    let l = ListaParaExportar::del_documento(COMPRAS, "x");
    assert_eq!(l.titulo, "Compras <de> la \"obra\" & más");
    assert_eq!(l.cuenta(), (2, 6));
    let t = &l.tareas[0];
    assert_eq!(t.titulo, "Llamar al banco");
    assert_eq!(t.urgencia, Urgencia::MuyUrgente);
    assert_eq!(t.etiquetas, ["casa"]);
    assert_eq!(t.nota.as_deref(), Some("no abren hasta el lunes"));
    assert_eq!(
        t.hora,
        Some(Momento {
            dia: fechas::dias_de(2026, 11, 2),
            minuto: 17 * 60
        })
    );
    assert_eq!(
        t.creada.map(|f| f.to_string()).as_deref(),
        Some("2026-10-28")
    );
    let y = &l.tareas[1];
    assert_eq!(y.titulo, "Comprar yeso y <cinta> de carrocero");
    assert_eq!(y.imagenes, ["pixpin:files/a/yeso.png"]);
    assert_eq!(y.urgencia, Urgencia::Urgente);
    // Sin titulo propio, el que se da.
    let sin = ListaParaExportar::del_documento("- [ ] a", "Lista");
    assert_eq!(sin.titulo, "Lista");
    assert_eq!(cuenta(&listas()), (5, 11));
    assert_eq!(por_ciento(5, 11), 45);
    assert_eq!(por_ciento(0, 0), 0);
}

#[test]
fn el_recordatorio_dice_hoy_manana_y_el_dia_con_su_fecha() {
    let r = Rotulos::default();
    let a = ahora();
    let m = |d: i64, min: u32| Momento {
        dia: a.dia + d,
        minuto: min,
    };
    assert_eq!(
        recordatorio(m(1, 17 * 60), a, &r),
        ("Mañana 17:00".into(), Some("2 de noviembre".into()))
    );
    assert_eq!(recordatorio(m(0, 20 * 60 + 30), a, &r).0, "Hoy 20:30");
    assert_eq!(
        recordatorio(m(5, 9 * 60), a, &r),
        ("vie 6 de noviembre 09:00".into(), None)
    );
    // Otro año: con el año.
    assert_eq!(
        recordatorio(m(70, 9 * 60), a, &r).0,
        "dom 10 de enero de 2027 09:00"
    );
    assert_eq!(
        r.exportado(a),
        "Exportado el 1 de noviembre de 2026 · 09:12"
    );
    let l = ListaParaExportar::del_documento(COMPRAS, "x");
    assert!(pasada(&l.tareas[3], a), "el seguro ya paso");
    assert!(!pasada(&l.tareas[0], a));
    // Caso negativo: lo hecho no esta «pasado».
    let v = ListaParaExportar::del_documento(HECHA, "x");
    assert!(!pasada(&v.tareas[1], a));
}

#[test]
fn la_pagina_lleva_el_resumen_las_listas_y_lo_hecho_plegado_al_final() {
    let r = Rotulos::default();
    let h = html(&listas(), "Mis tareas", &r, ahora(), &foto);
    assert!(h.starts_with("<!DOCTYPE html>"));
    assert!(h.contains("<html lang=\"es\">"));
    assert!(h.contains("<title>Mis tareas</title>"));
    assert!(h.contains("prefers-color-scheme: dark"));
    assert!(h.contains("@media print"));
    // Nada de la red.
    assert!(!h.contains("http://") && !h.contains("https://"));
    // El resumen: 6 pendientes, 5 hechas, 45 %.
    assert!(h.contains("<b>6</b><span>Pendientes</span>"));
    assert!(h.contains("<b>5</b><span>Hechas</span>"));
    assert!(h.contains("<b>45<small>%</small></b>"));
    assert!(h.contains("aria-valuenow=\"5\""));
    // Una tarjeta por lista, con su cabecera (son varias).
    assert_eq!(h.matches("<section class=\"lista\">").count(), 3);
    assert_eq!(h.matches("<header class=\"lista-cab\">").count(), 3);
    assert!(h.contains("<span class=\"cuenta\">2 de 6</span>"));
    // Las filas: once tareas, cinco hechas, y lo hecho dentro del plegado.
    assert_eq!(h.matches("<li class=\"tarea").count(), 11);
    assert_eq!(h.matches("<li class=\"tarea hecha").count(), 5);
    assert_eq!(h.matches("<details class=\"hechas\">").count(), 3);
    assert!(!h.contains("<details class=\"hechas\" open"));
    let i_hechas = h.find("<details class=\"hechas\">").unwrap();
    assert!(h.find("Cemento").unwrap() > i_hechas);
    assert!(h.find("Llamar al banco").unwrap() < i_hechas);
    // Las chapitas.
    assert!(h.contains("chip prio muy"));
    assert!(h.contains("chip prio alta"));
    assert!(h.contains("<time datetime=\"2026-11-02T17:00\">Mañana 17:00</time>"));
    assert!(h.contains("<span class=\"lejos\">2 de noviembre</span>"));
    assert!(h.contains("chip hora pasada"));
    assert!(h.contains("<span class=\"chip tag\">#casa</span>"));
    assert!(h.contains("<p class=\"nota\">no abren hasta el lunes</p>"));
    assert!(h.contains("Creada el 28 de octubre"));
    assert!(h.contains("<img src=\"data:image/png;base64,"));
    // Una lista con todo hecho lo dice.
    assert!(h.contains("<p class=\"vacio\">Todo hecho</p>"));
    // Ni marcas crudas ni enlaces de las fotos.
    for crudo in [
        "⏫",
        "🔼",
        "➕",
        "💬",
        "pixpin:",
        "- [ ]",
        "2026-11-02 17:00",
    ] {
        assert!(!h.contains(crudo), "se colo {crudo:?}");
    }
}

#[test]
fn lo_que_escribe_el_usuario_va_escapado() {
    let r = Rotulos::default();
    let doc =
        "# <script>alert(1)</script>\n\n- [ ] <img src=x onerror=alert(2)> \"y\" & 'z' #<b>\n";
    let l = ListaParaExportar::del_documento(doc, "x");
    let h = html(std::slice::from_ref(&l), &l.titulo, &r, ahora(), &|_| None);
    assert!(!h.contains("<script>alert"));
    assert!(!h.contains("<img src=x"));
    assert!(h.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(h.contains("&lt;img src=x onerror=alert(2)&gt; &quot;y&quot; &amp; &#39;z&#39;"));
    // Con una lista sola no hay cabecera de lista: el titulo es el suyo.
    assert!(!h.contains("<header class=\"lista-cab\">"));
    // Y una foto que no se da no sale.
    let y = ListaParaExportar::del_documento(COMPRAS, "x");
    let h = html(&[y], "", &r, ahora(), &|_| None);
    assert!(!h.contains("<img"));
    assert!(h.contains("<title>Tareas</title>"));
    // El Markdown escapa lo que cambiaria su forma.
    let m = markdown(&[l], "x", &r, ahora());
    assert!(m.contains("\\<img src=x onerror=alert(2)\\>"));
}

#[test]
fn el_markdown_y_el_texto_llevan_casillas_y_un_sufijo_compacto() {
    let r = Rotulos::default();
    let m = markdown(&listas(), "Mis tareas", &r, ahora());
    assert!(m.starts_with("# Mis tareas\n\n*Exportado el 1 de noviembre de 2026 · 09:12 · Pendientes: 6 · Hechas: 5 · 45 %*\n"));
    assert!(m.contains("\n## Compras \\<de\\> la \"obra\" & más — 2 de 6\n"));
    assert!(m.contains(
        "- [ ] Llamar al banco · **Muy urgente** · ⏰ Mañana 17:00 (2 de noviembre) · #casa — *no abren hasta el lunes*\n"
    ));
    assert!(m.contains(
        "- [ ] Renovar el seguro del coche · **Urgente** · ⏰ vie 30 de octubre 10:00 · Pasada\n"
    ));
    assert!(m.contains("\n### Hechas (2)\n\n- [x] Cemento\n- [x] Arena de río · #obra\n"));
    assert!(m.contains("*Todo hecho*"));
    assert!(!m.contains("pixpin:") && !m.contains("➕"));
    let t = texto(&listas(), "Mis tareas", &r, ahora());
    assert!(t.contains("☐ Llamar al banco · Muy urgente · ⏰ Mañana 17:00 (2 de noviembre) · #casa\n   💬 no abren hasta el lunes\n"));
    assert!(t.contains("\nHechas (2)\n☑ Cemento\n"));
    // Una lista sola: sin titulo de lista y lo hecho en un segundo nivel.
    let una = markdown(&listas()[..1], "Compras", &r, ahora());
    assert!(!una.contains("\n## Compras"));
    assert!(una.contains("\n## Hechas (2)\n"));
}

#[test]
fn las_hojas_dibujan_casillas_chapitas_y_fotos_y_parten_lo_largo() {
    let r = Rotulos::default();
    let mut pedidas = Vec::new();
    let mut f = |e: &str| {
        pedidas.push(e.to_string());
        Some((7u64, 400.0, 300.0))
    };
    let h = hojas(&listas(), "Mis tareas", &r, ahora(), &mut f);
    assert!(!h.is_empty() && h.len() <= 2, "{} hojas", h.len());
    assert_eq!(pedidas, ["pixpin:files/a/yeso.png"]);
    let textos: Vec<&str> = h
        .iter()
        .flat_map(|x| &x.ordenes)
        .filter_map(|o| match o {
            Orden::Texto { texto, .. } => Some(texto.as_str()),
            _ => None,
        })
        .collect();
    for esperado in [
        "Mis tareas",
        "Llamar al banco",
        "Muy urgente",
        "Mañana 17:00 · 2 de noviembre",
        "#casa",
        "no abren hasta el lunes",
        "HECHAS · 2",
        "Cemento",
        "2 de 6",
    ] {
        assert!(textos.contains(&esperado), "falta {esperado:?}: {textos:?}");
    }
    assert!(
        h[0].ordenes
            .iter()
            .any(|o| matches!(o, Orden::Imagen { id_objeto: 7, .. }))
    );
    // Todo dentro del papel.
    for o in h.iter().flat_map(|x| &x.ordenes) {
        if let Orden::Texto { x, y, .. } = o {
            assert!(*x >= 0.0 && *x < ANCHO_A4 && *y >= 0.0 && *y < ALTO_A4);
        }
    }
    // Una lista larga pasa de hoja.
    let mut doc = String::from("# Larga\n\n");
    for i in 0..80 {
        doc.push_str(&format!(
            "- [ ] Tarea numero {i} ⏰ 2026-11-0{} 10:00 #casa\n",
            1 + i % 8
        ));
    }
    let l = ListaParaExportar::del_documento(&doc, "x");
    let h = hojas(&[l], "Larga", &r, ahora(), &mut |_| None);
    assert!(h.len() >= 3, "{} hojas", h.len());
    for hoja in &h {
        for o in &hoja.ordenes {
            if let Orden::Texto { y, .. } = o {
                assert!(*y < ALTO_A4 - MARGEN + 1.0);
            }
        }
    }
}

/// Deja las muestras en `PINDESK_MUESTRAS_TAREAS` para mirarlas.
#[test]
fn muestras_para_mirar() {
    let Some(dir) = std::env::var_os("PINDESK_MUESTRAS_TAREAS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let r = Rotulos::default();
    let todas = listas();
    let una = &todas[..1];
    std::fs::write(
        dir.join("varias.html"),
        html(&todas, "Mis tareas", &r, ahora(), &foto),
    )
    .unwrap();
    std::fs::write(
        dir.join("una.html"),
        html(una, &una[0].titulo, &r, ahora(), &foto),
    )
    .unwrap();
    std::fs::write(
        dir.join("varias.md"),
        markdown(&todas, "Mis tareas", &r, ahora()),
    )
    .unwrap();
    std::fs::write(
        dir.join("varias.txt"),
        texto(&todas, "Mis tareas", &r, ahora()),
    )
    .unwrap();
    let foto_svg = |id: u64| {
        (id == 7).then(|| pindesk_motor2d::exportar_svg::Incrustada {
            mime: "image/png",
            bytes: pindesk_motor2d::exportar::png_rgba(2, 2, &[180, 160, 140, 255].repeat(4)),
        })
    };
    for (nombre, l, t) in [
        ("varias", &todas[..], "Mis tareas"),
        ("una", una, "Compras"),
    ] {
        let h = hojas(l, t, &r, ahora(), &mut |_| Some((7, 400.0, 300.0)));
        for (i, hoja) in h.iter().enumerate() {
            let svg = pindesk_motor2d::exportar_svg::svg(
                hoja,
                pindesk_motor2d::exportar_svg::OpcionesSvg {
                    fondo: Some(BLANCO),
                    marcos: false,
                },
                &foto_svg,
            );
            std::fs::write(dir.join(format!("{nombre}-hoja{}.svg", i + 1)), svg).unwrap();
        }
    }
}
