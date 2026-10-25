//! La voz del asistente: lo que se lee de una respuesta (sin pensamiento ni
//! JSON), cuando el boton es el microfono del asistente, lo dicho que
//! llega a la caja y se envia (o se queda para repasarlo), los fallos
//! (silencio, sin microfono) y, aparte (`--ignored`), Whisper de verdad
//! con un WAV sintetico y el PNG del boton.

use super::*;
use pixpin_ia::motor::{MotorGuionado, Respuesta};
use pixpin_store::idioma::Idioma;

fn chat(nombre: &str, del_asistente: bool) -> (Ubicacion, Abierto, PathBuf) {
    olvidar_para_pruebas();
    asistente::olvidar_para_pruebas();
    let raiz = std::env::temp_dir().join(format!(
        "pixpin-voz-asistente-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&raiz);
    std::fs::create_dir_all(&raiz).unwrap();
    let u = Ubicacion::Portable { raiz: raiz.clone() };
    let ficha = if del_asistente {
        pixpin_proyecto::almacen::asegurar_asistente(&raiz, 0, "PC").unwrap()
    } else {
        let f = pixpin_proyecto::almacen::Ficha::nueva("obra", 0, "PC");
        let mut i = pixpin_proyecto::almacen::Indice::leer(&raiz);
        i.proyectos.push(f.clone());
        i.guardar(&raiz).unwrap();
        f
    };
    std::fs::create_dir_all(pixpin_proyecto::almacen::carpeta(&raiz, &ficha.id)).unwrap();
    let a = abrir_proyecto(&u, &ficha);
    (u, a, raiz)
}

fn textos() -> Catalogo {
    Catalogo::nuevo(Idioma::Espanol)
}

/// Vueltas al latido de la voz hasta que llegue algo.
fn hasta_que_llegue(u: &Ubicacion, a: &mut Abierto, t: &Catalogo) -> Vuelta {
    let hasta = Instant::now() + Duration::from_secs(5);
    while Instant::now() < hasta {
        let v = latido(u, Some(a), t);
        if v.aviso.is_some() || v.enviar || !pasando(&a.ficha.id) {
            return v;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("no llego nada");
}

/// Lo que hace el bucle del chat con `Vuelta::enviar`.
fn enviar_como_el_bucle(u: &Ubicacion, a: &mut Abierto, motor: Arc<dyn pixpin_ia::motor::Motor>) {
    guardar_nota(u, a, "K7Q2").unwrap();
    let pregunta =
        ia_chat::pregunta_de(a.mensajes.last().unwrap(), a.ficha.es_asistente()).unwrap();
    asistente::lanzar(u, a, motor, pregunta).unwrap();
    let hasta = Instant::now() + Duration::from_secs(20);
    while asistente::espera().is_some() {
        assert!(Instant::now() < hasta, "el asistente no acabo");
        asistente::latido(u, Some(a));
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ------------------------------------------------------------- lo que se lee

#[test]
fn se_lee_la_respuesta_sin_pensamiento_ni_json_ni_estados() {
    let burbuja = "<think>El usuario quiere una casa. Uso dibujar_en_lienzo.</think>\n\
        ✏️ dibujando en el lienzo…\n\
        {\"name\": \"dibujar_en_lienzo\", \"arguments\": {\"elementos\": []}}\n\
        ## Hecho\n\
        Te dibujé **una casa** con dos ventanas 🏠.\n\
        - Mira el [lienzo](https://ejemplo.com/x) cuando quieras.\n\
        ```json\n{\"tipo\": \"rectangulo\"}\n```\n\
        | Pieza | Cantidad |\n|---|---|\n| Ventanas | 2 |\n\
        ⚠️ algo falló por el camino";
    let t = texto_para_leer(burbuja);
    assert_eq!(
        t,
        "Hecho\nTe dibujé una casa con dos ventanas.\nMira el lienzo cuando quieras.\nPieza, Cantidad\nVentanas, 2"
    );
    for nada in [
        "think",
        "dibujar_en_lienzo",
        "{",
        "rectangulo",
        "https",
        "⚠",
        "**",
        "🏠",
    ] {
        assert!(!t.contains(nada), "se colo «{nada}» en {t:?}");
    }
    assert_eq!(parrafos_para_leer(burbuja).len(), 5);
}

#[test]
fn pensamiento_sin_cerrar_o_cierre_suelto_y_parado() {
    assert_eq!(
        texto_para_leer("Hola.<think>sigo pensando y no cierro"),
        "Hola."
    );
    assert_eq!(
        texto_para_leer("pensado a medias</think>La respuesta."),
        "La respuesta."
    );
    assert_eq!(
        texto_para_leer("Iba a decir algo ⏹ (parado)"),
        "Iba a decir algo"
    );
    assert_eq!(texto_para_leer("<thinking>x</thinking>Sí."), "Sí.");
}

#[test]
fn una_burbuja_solo_de_estado_o_de_aviso_no_tiene_nada_que_leer() {
    assert!(texto_para_leer("💭 …").is_empty());
    assert!(texto_para_leer(" ▍").is_empty());
    assert!(texto_para_leer("⚠️ No hay conexión con la nube.").is_empty());
    assert!(texto_para_leer("<think>solo pensé</think>").is_empty());
    assert!(texto_para_leer("```\nfn main() {}\n```").is_empty());
    assert!(parrafos_para_leer("").is_empty());
}

#[test]
fn lo_oido_sin_los_rotulos_que_whisper_se_inventa() {
    let t = limpiar_lo_oido(
        [
            "[Música]",
            " dibuja  una casa ",
            "(risas)",
            "con dos ventanas",
        ]
        .into_iter(),
    );
    assert_eq!(t, "dibuja una casa con dos ventanas");
    assert_eq!(limpiar_lo_oido(["[Silencio]", "  "].into_iter()), "");
    assert_eq!(juntar("@asistente", "hola"), "@asistente hola");
    assert_eq!(juntar("", "hola"), "hola");
}

#[test]
fn los_fallos_en_palabras_del_usuario() {
    let t = textos();
    assert_eq!(
        clave_del_fallo(&ErrorVoz::NoSeEntiendeNada),
        "asistente-voz-nada"
    );
    assert_eq!(clave_del_fallo(&ErrorVoz::AudioVacio), "asistente-voz-nada");
    assert_eq!(
        clave_del_fallo(&ErrorVoz::Cancelada),
        "asistente-voz-cancelado"
    );
    assert_eq!(
        clave_del_fallo(&ErrorVoz::SinMicrofono),
        "chat-voz-sin-microfono"
    );
    // Sin microfono al empezar: el mismo aviso que las notas de voz.
    assert_eq!(
        rotulo_de_no_grabar(&pixpin_audio::ErrorAudio::SinMicrofono),
        "chat-voz-sin-microfono"
    );
    for clave in [
        "asistente-voz-nada",
        "asistente-voz-cancelado",
        "asistente-voz-fallo",
        "asistente-voz-sin-whisper",
        "asistente-voz-nada-que-leer",
        "chat-voz-sin-microfono",
    ] {
        assert_ne!(t.t(clave), clave, "falta el texto de {clave}");
    }
}

// ------------------------------------------------------------- el microfono

#[test]
fn el_boton_es_el_microfono_del_asistente_solo_donde_toca() {
    let (_u, mut a, raiz) = chat("boton", true);
    assert!(microfono_del_asistente(&a), "chat Asistente, caja vacia");
    a.borrador = "hola".into();
    assert!(!microfono_del_asistente(&a), "con algo escrito, enviar");
    a.borrador = " @Asistente ".into();
    assert!(microfono_del_asistente(&a));
    let _ = std::fs::remove_dir_all(&raiz);

    let (_u, mut a, raiz) = chat("boton-otro", false);
    assert!(
        !microfono_del_asistente(&a),
        "en otro chat el microfono graba notas de voz"
    );
    a.borrador = "@asistente".into();
    assert!(
        microfono_del_asistente(&a),
        "con solo la mencion, se le habla"
    );
    let _tx = pasando_para_pruebas(&a.ficha.id, "@asistente", 0.3);
    assert!(
        !microfono_del_asistente(&a),
        "pasando a texto, el boton cancela"
    );
    assert!(!grabando_para_el_asistente(&a));
    olvidar_para_pruebas();
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn lo_dicho_llega_a_la_caja_y_se_envia_solo() {
    let (u, mut a, raiz) = chat("enviar", true);
    let t = textos();
    let tx = pasando_para_pruebas(&a.ficha.id, "", 0.5);
    assert!(rotulo_pasando(&a, &t).unwrap().contains("50"));
    assert_eq!(espera(), Some(250));
    tx.send(Ok("dibuja una casa".into())).unwrap();
    let v = hasta_que_llegue(&u, &mut a, &t);
    assert!(v.enviar && v.aviso.is_none());
    assert_eq!(a.borrador, "dibuja una casa");
    assert!(!pasando(&a.ficha.id));

    // Y sigue como un mensaje escrito: con cualquier motor (aqui uno de
    // mentira; la voz no sabe cual contesta).
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(
        "<think>me piden una casa</think>Claro: una casa con tejado rojo.",
    )]));
    enviar_como_el_bucle(&u, &mut a, motor);
    let i = a.mensajes.len() - 1;
    assert!(ia_chat::es_del_asistente(&a.mensajes[i]));
    assert!(se_puede_leer(&a, i), "la respuesta lleva altavoz");
    assert!(!se_puede_leer(&a, i - 1), "la pregunta no");
    assert_eq!(
        parrafos_para_leer(&a.mensajes[i].texto),
        vec!["Claro: una casa con tejado rojo."]
    );
    // Acabo: queda apuntada para «leer siempre» (apagado: no se lee).
    let v = latido(&u, Some(&mut a), &t);
    assert!(v.aviso.is_none());
    assert!(!leyendo(&a.ficha.id, &a.mensajes[i].id));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn con_revisar_lo_dicho_se_queda_en_la_caja_tras_la_mencion() {
    let (u, mut a, raiz) = chat("revisar", false);
    let mut aj = pixpin_store::Ajustes::default();
    aj.asistente.revisar_lo_dicho = true;
    pixpin_store::guardar(&u, &aj).unwrap();
    let t = textos();
    a.borrador = "@asistente".into();
    let tx = pasando_para_pruebas(&a.ficha.id, "@asistente", 0.0);
    tx.send(Ok("qué hay en el plano".into())).unwrap();
    let v = hasta_que_llegue(&u, &mut a, &t);
    assert!(!v.enviar, "se repasa antes de enviar");
    assert_eq!(a.borrador, "@asistente qué hay en el plano");
    assert_eq!(
        a.cursor.pos(&a.borrador),
        a.borrador.len(),
        "el cursor al final"
    );
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn silencio_cancelar_o_hilo_muerto_avisan_y_no_envian_nada() {
    let (u, mut a, raiz) = chat("fallos", true);
    let t = textos();
    for (r, clave) in [
        (Some(Err(ErrorVoz::NoSeEntiendeNada)), "asistente-voz-nada"),
        (Some(Err(ErrorVoz::Cancelada)), "asistente-voz-cancelado"),
        (None, "asistente-voz-cancelado"),
    ] {
        let tx = pasando_para_pruebas(&a.ficha.id, "", 0.9);
        match r {
            Some(r) => tx.send(r).unwrap(),
            None => drop(tx),
        }
        let v = hasta_que_llegue(&u, &mut a, &t);
        assert_eq!(v.aviso.as_deref(), Some(t.t(clave).as_str()));
        assert!(!v.enviar);
        assert!(a.borrador.is_empty(), "no se escribe nada");
        assert!(a.mensajes.is_empty(), "ni se manda nada");
    }
    // Cancelar marca el hilo para que pare.
    let _tx = pasando_para_pruebas(&a.ficha.id, "", 0.2);
    cancelar();
    assert!(PASANDO.with_borrow(|p| p.as_ref().unwrap().cancelado.load(Ordering::Relaxed)));
    olvidar_para_pruebas();
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn lo_dicho_para_otro_chat_no_cae_en_el_abierto() {
    let (u, mut a, raiz) = chat("otro", true);
    let t = textos();
    let tx = pasando_para_pruebas("otro-proyecto", "", 0.0);
    tx.send(Ok("hola".into())).unwrap();
    let v = latido(&u, Some(&mut a), &t);
    assert!(!v.enviar);
    assert!(a.borrador.is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn terminar_sin_grabar_no_hace_nada() {
    let (u, mut a, raiz) = chat("sin-grabar", true);
    assert_eq!(terminar(&u, &mut a, Idioma::Espanol, &textos()), None);
    assert!(!pasando(&a.ficha.id));
    assert!(!al_soltar(&a));
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
fn una_respuesta_sin_texto_avisa_en_vez_de_leer() {
    let (u, mut a, raiz) = chat("sin-texto", true);
    let n = escribir_nota(
        &raiz,
        &a.ficha.id,
        ia_chat::APARATO,
        "⚠️ Sin red.",
        1,
        1,
        None,
    )
    .unwrap();
    a.vistas.push(None);
    a.mensajes.push(n);
    assert!(!se_puede_leer(&a, 0));
    assert_eq!(
        alternar(&u, &a, 0, &textos()),
        Some(textos().t("asistente-voz-nada-que-leer"))
    );
    assert!(LEYENDO.with_borrow(Option::is_none));
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------- con Whisper de verdad

/// Una frase dicha por la voz de Windows, a WAV de 16 kHz mono, con
/// System.Speech (lo trae Windows). `None` si no hay voz castellana.
fn wav_sintetico(frase: &str, destino: &Path) -> Option<()> {
    let guion = format!(
        "Add-Type -AssemblyName System.Speech; \
         $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         $v = $s.GetInstalledVoices() | Where-Object {{ $_.VoiceInfo.Culture.TwoLetterISOLanguageName -eq 'es' }} | Select-Object -First 1; \
         if (-not $v) {{ exit 3 }}; \
         $s.SelectVoice($v.VoiceInfo.Name); $s.Rate = -1; \
         $f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono); \
         $s.SetOutputToWaveFile('{}', $f); $s.Speak('{}'); $s.Dispose()",
        destino.display(),
        frase
    );
    let st = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &guion])
        .status()
        .ok()?;
    (st.success() && destino.is_file()).then_some(())
}

/// Un WAV de 16 kHz mono con estas muestras.
fn wav_de(muestras: &[i16], destino: &Path) {
    let datos = (muestras.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + datos as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + datos).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&16_000u32.to_le_bytes());
    b.extend_from_slice(&32_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&datos.to_le_bytes());
    for m in muestras {
        b.extend_from_slice(&m.to_le_bytes());
    }
    std::fs::write(destino, b).unwrap();
}

/// La carpeta de datos con `whisper/base/` (la de la app instalada si no se
/// dice otra): solo se lee.
fn raiz_con_whisper() -> Option<PathBuf> {
    let r = std::env::var_os("PIXPIN_VOZ_RAIZ")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("PixPinMax")))?;
    pixpin_voz::whisper::modelo_listo(&r).then_some(r)
}

/// Un chat cuya carpeta de datos tiene una copia del modelo de Whisper.
fn chat_con_whisper(nombre: &str, de: &Path) -> (Ubicacion, Abierto, PathBuf) {
    let (u, a, raiz) = chat(nombre, true);
    let origen = pixpin_voz::whisper::carpeta(de);
    let destino = pixpin_voz::whisper::carpeta(&raiz);
    std::fs::create_dir_all(&destino).unwrap();
    for f in std::fs::read_dir(&origen).unwrap().flatten() {
        std::fs::copy(f.path(), destino.join(f.file_name())).unwrap();
    }
    (u, a, raiz)
}

/// ```text
/// cargo test -p pixpin --bin pixpinmax -- --ignored --test-threads=1 whisper_de_verdad
/// ```
#[test]
#[ignore = "pide el modelo de Whisper bajado y la voz castellana de Windows"]
fn whisper_de_verdad_wav_conocido_a_texto_y_enviado() {
    let Some(de) = raiz_con_whisper() else {
        panic!("no hay modelo de Whisper (PIXPIN_VOZ_RAIZ)");
    };
    let (u, mut a, raiz) = chat_con_whisper("whisper", &de);
    let wav = raiz.join("dicho.wav");
    wav_sintetico("Dibuja una casa con dos ventanas y una puerta.", &wav)
        .expect("voz castellana de Windows");
    let t = textos();
    let t0 = Instant::now();
    assert_eq!(
        lanzar(
            &u,
            &a.ficha.id.clone(),
            String::new(),
            wav.clone(),
            Idioma::Espanol,
            &t
        ),
        None
    );
    assert!(pasando(&a.ficha.id));
    let hasta = Instant::now() + Duration::from_secs(120);
    let v = loop {
        let v = latido(&u, Some(&mut a), &t);
        if v.enviar || v.aviso.is_some() {
            break v;
        }
        assert!(Instant::now() < hasta, "Whisper no acabo");
        std::thread::sleep(Duration::from_millis(50));
    };
    eprintln!(
        "oido en {:.1} s: «{}»",
        t0.elapsed().as_secs_f32(),
        a.borrador
    );
    assert_eq!(v.aviso, None);
    assert!(v.enviar);
    let oido = a.borrador.to_lowercase();
    for palabra in ["casa", "ventanas", "puerta"] {
        assert!(oido.contains(palabra), "falta «{palabra}» en «{oido}»");
    }
    assert!(!wav.exists(), "lo grabado se borra al acabar");
    let _ = std::fs::remove_dir_all(&raiz);
}

#[test]
#[ignore = "pide el modelo de Whisper bajado"]
fn whisper_de_verdad_el_silencio_no_se_envia() {
    let Some(de) = raiz_con_whisper() else {
        panic!("no hay modelo de Whisper (PIXPIN_VOZ_RAIZ)");
    };
    let (u, mut a, raiz) = chat_con_whisper("silencio", &de);
    let wav = raiz.join("silencio.wav");
    // Tres segundos de casi nada: el soplo de un microfono abierto.
    let muestras: Vec<i16> = (0..48_000).map(|i| ((i * 7919) % 13) as i16 - 6).collect();
    wav_de(&muestras, &wav);
    let t = textos();
    assert_eq!(
        lanzar(
            &u,
            &a.ficha.id.clone(),
            String::new(),
            wav,
            Idioma::Espanol,
            &t
        ),
        None
    );
    let hasta = Instant::now() + Duration::from_secs(120);
    let v = loop {
        let v = latido(&u, Some(&mut a), &t);
        if v.enviar || v.aviso.is_some() {
            break v;
        }
        assert!(Instant::now() < hasta, "Whisper no acabo");
        std::thread::sleep(Duration::from_millis(50));
    };
    eprintln!("silencio: aviso {:?}, caja «{}»", v.aviso, a.borrador);
    assert!(!v.enviar, "el silencio no se manda");
    assert_eq!(v.aviso.as_deref(), Some(t.t("asistente-voz-nada").as_str()));
    assert!(a.borrador.is_empty());
    let _ = std::fs::remove_dir_all(&raiz);
}

// ------------------------------------------------------------- el PNG

/// El chat «Asistente» con una respuesta (su altavoz junto a la hora) y la
/// caja con el microfono del asistente; y otra con lo dicho pasando a
/// texto.
///
/// ```text
/// PIXPIN_MUESTRAS=<carpeta> cargo test -p pixpin --bin pixpinmax -- --ignored --test-threads=1 muestra_de_la_voz_del_asistente
/// ```
#[test]
#[ignore = "necesita GPU; genera PNG para mirarlos"]
fn muestra_de_la_voz_del_asistente() {
    use pixpin_render::MotorRender;
    use pixpin_render::fuera_de_pantalla::FueraDePantalla;
    let (u, mut a, raiz) = chat("muestra", true);
    let motor = Arc::new(MotorGuionado::nuevo(vec![Respuesta::de_texto(
        "<think>Una receta corta.</think>Para dos personas: 200 g de arroz, un pimiento y caldo. Sofríe el pimiento, añade el arroz y cuece 18 minutos.",
    )]));
    a.borrador = "¿Qué hago de cena con arroz?".into();
    enviar_como_el_bucle(&u, &mut a, motor);

    let textos = textos();
    let ahora = pixpin_shell::entorno::ahora_utc_ms();
    let d3 = pixpin_capture::Dispositivo::nuevo().expect("GPU real");
    let motor_r = MotorRender::nuevo(d3.d3d()).expect("motor");
    let (ancho, alto) = (900u32, 560u32);
    let destino = FueraDePantalla::nuevo(&motor_r, d3.d3d(), ancho, alto).expect("destino");
    let escala = 100;
    let marco = Rect {
        x: 0,
        y: 0,
        ancho,
        alto,
    };
    let miniaturas = crate::miniaturas::Miniaturas::nuevo();
    let previas = crate::miniaturas::Miniaturas::con_lado(PREVIA_LADO);
    let fichas = vec![a.ficha.clone()];
    let orden = vec![0usize];
    let marcados = Default::default();
    let salida = std::env::var_os("PIXPIN_MUESTRAS")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&salida).unwrap();

    for (nombre, pasando_ahora) in [
        ("asistente-voz-microfono.png", false),
        ("asistente-voz-pasando.png", true),
    ] {
        let _tx = pasando_ahora.then(|| pasando_para_pruebas(&a.ficha.id, "", 0.42));
        let d = disponer(
            marco,
            escala,
            pixpin_ui::chat::ancho_inicial(ancho, escala),
            Some(&a),
        );
        for _ in 0..3 {
            let lista = Lista {
                fichas: &fichas,
                orden: &orden,
                scroll: 0,
                sobre: None,
                elegida: Some(0),
                ahora,
                textos: &textos,
                busqueda: &crate::caja_texto::CajaLinea::nueva(),
                buscando: false,
                marcados: &marcados,
                raiz: std::path::Path::new(""),
                giro_sincro: 0.0,
                sincro_fallo: false,
                en_chats: None,
                sobre_mensaje: None,
            };
            motor_r
                .dibujar(&destino.destino, |p| {
                    pintar(
                        p,
                        &d,
                        &CLARO,
                        papel_de(0, true),
                        escala,
                        &textos,
                        None,
                        &lista,
                    );
                    a.zonas.borrow_mut().clear();
                    let c = Pinta {
                        tema: &CLARO,
                        escala,
                        textos: &textos,
                        ahora,
                        miniaturas: &miniaturas,
                        previas: &previas,
                    };
                    let alto_texto = (pixpin_ui::chat::REDACCION_TAM * 1.3).ceil() as u32;
                    a.alto_caja.set(alto_texto);
                    pintar_historial(p, &d, &c, &a);
                    pintar_redaccion(p, &d, &c, &a, alto_texto);
                })
                .expect("fotograma");
            destino.esperar_gpu().expect("GPU");
        }
        // El altavoz es pulsable y es de la respuesta.
        let i = a.mensajes.len() - 1;
        assert!(
            a.zonas
                .borrow()
                .iter()
                .any(|(_, z)| *z == Zona::LeerRespuesta(i)),
            "falta el altavoz de la respuesta"
        );
        let (w, hh, pixeles) = destino.leer_rgba().expect("leer");
        let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba {
            ancho: w,
            alto: hh,
            pixeles,
        })
        .unwrap();
        let ruta = salida.join(nombre);
        std::fs::write(&ruta, png).unwrap();
        println!("{}", ruta.display());
        olvidar_para_pruebas();
    }
    let _ = std::fs::remove_dir_all(&raiz);
}
