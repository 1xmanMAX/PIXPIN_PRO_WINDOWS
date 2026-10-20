//! **Los Word y PowerPoint de antes, en el visor propio** (20-oct-2026). El
//! usuario: «dentro del chat también tenemos archivos de Office como Word,
//! Excel, PowerPoint. Entonces eso sí se tienen que sincronizar». Un `.doc`,
//! un `.ppt` (y sus primos, el RTF y el OpenDocument) se veian en el pin de
//! Office con la vista previa del Explorador, que no dice por donde va: sin
//! tinta pegada a su sitio, sin marcos ni marcadores, y nada que viaje.
//!
//! Ahora se convierten **una vez** a `.docx`/`.pptx` con el propio Office (por
//! COM, sin ventana, y Office se cierra al acabar: no se queda en segundo
//! plano) y se leen en el visor propio. La copia vive en la cache
//! (`<raiz>/cache/office/<clave>/`), por la ruta, el tamano y la fecha del
//! original: si el original cambia, se vuelve a convertir.
//!
//! Lo anotado sobre la copia es **lo del original**: se lee en su nombre
//! (`pixpin_proyecto::anotado::suplir`), asi que de un adjunto del chat va a
//! sus `anot-<uid>…` y viaja como lo de un `.docx`.
//!
//! La primera vez se abre el pin de Office al instante (como antes) y la
//! conversion va detras; cuando esta, el pin se cambia solo al visor propio.
//! Sin Office en el equipo, o si no se deja convertir (con contrasena, roto),
//! se queda el pin de Office.
//!
//! Este modulo no lleva `unsafe`: Office se maneja desde un PowerShell
//! escondido.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

/// Lo mas que se espera a Office (arranca cada vez).
const TOPE: Duration = Duration::from_secs(90);

/// A que formato se convierte cada extension (sin punto, en minusculas):
/// `None` si no se convierte (Excel ya lo lee el visor de hojas; Visio y los
/// correos no tienen visor propio).
pub fn destino(ext: &str) -> Option<&'static str> {
    match ext {
        "doc" | "dot" | "rtf" | "odt" => Some("docx"),
        "ppt" | "pps" | "pot" | "odp" => Some("pptx"),
        _ => None,
    }
}

/// **Donde va la copia** de `original` en la cache: cambia si cambia el
/// original (ruta, tamano o fecha). `None` si no se convierte o no se lee.
pub fn copia_de(raiz: &Path, original: &Path) -> Option<PathBuf> {
    let ext = pixpin_shell::vista_previa::extension_de(original);
    let a = destino(&ext)?;
    let clave = crate::plano_cad::clave(original)?;
    let tronco = pixpin_docs::sin_extension(&pixpin_docs::nombre(original));
    Some(raiz.join("cache").join("office").join(clave).join(format!("{tronco}.{a}")))
}

/// La copia ya convertida, si esta: abrirla es inmediato. Queda leida en
/// nombre del original.
pub fn lista(raiz: &Path, original: &Path) -> Option<PathBuf> {
    let c = copia_de(raiz, original).filter(|c| c.is_file())?;
    pixpin_proyecto::anotado::suplir(&c, original);
    Some(c)
}

/// **Convierte** `original` a su copia con Office (bloquea: va en su hilo).
/// Devuelve la copia, ya leida en nombre del original.
pub fn convertir(raiz: &Path, original: &Path) -> Result<PathBuf> {
    let copia = copia_de(raiz, original).context("no se convierte")?;
    if copia.is_file() {
        pixpin_proyecto::anotado::suplir(&copia, original);
        return Ok(copia);
    }
    let carpeta = copia.parent().context("sin carpeta")?;
    std::fs::create_dir_all(carpeta)?;
    let a = pixpin_shell::vista_previa::extension_de(&copia);
    // A medio escribir no vale: Office escribe en `.tmp` y se renombra.
    let medio = carpeta.join(format!("convirtiendo.{a}"));
    let _ = std::fs::remove_file(&medio);
    let guion = carpeta.join("convertir.ps1");
    std::fs::write(&guion, GUION)?;
    let desde = Instant::now();
    let mut orden = std::process::Command::new("powershell.exe");
    orden.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(&guion).arg(original).arg(&medio).arg(&a);
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: ni un parpadeo de consola.
        orden.creation_flags(0x0800_0000);
    }
    let mut hijo = orden.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().context("no se pudo lanzar PowerShell")?;
    let fin = loop {
        if let Some(s) = hijo.try_wait()? {
            break s;
        }
        if desde.elapsed() > TOPE {
            let _ = hijo.kill();
            let _ = std::fs::remove_file(&guion);
            anyhow::bail!("Office no termino en {} s", TOPE.as_secs());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = std::fs::remove_file(&guion);
    if !fin.success() || !medio.is_file() {
        let mut error = String::new();
        if let Some(mut e) = hijo.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut error);
        }
        let _ = std::fs::remove_file(&medio);
        anyhow::bail!("Office no lo convirtio: {}", error.trim());
    }
    std::fs::rename(&medio, &copia)?;
    tracing::info!(original = %original.display(), ms = desde.elapsed().as_millis() as u64, "Office viejo convertido para el visor propio");
    pixpin_proyecto::anotado::suplir(&copia, original);
    Ok(copia)
}

/// El guion de PowerShell: Word o PowerPoint por COM, sin ventana, solo
/// lectura y sin preguntar nada (una contrasena falsa hace fallar en vez de
/// pedirla); Office se cierra pase lo que pase.
const GUION: &str = r#"param([string]$Desde, [string]$Hasta, [string]$A)
$ErrorActionPreference = 'Stop'
if ($A -eq 'docx') {
  $app = New-Object -ComObject Word.Application
  try {
    $app.Visible = $false
    $app.DisplayAlerts = 0
    $doc = $app.Documents.Open($Desde, $false, $true, $false, 'pixpin-sin-clave')
    $doc.SaveAs2($Hasta, 16)
    $doc.Close(0)
  } finally { $app.Quit() }
} else {
  $app = New-Object -ComObject PowerPoint.Application
  try {
    $p = $app.Presentations.Open($Desde, -1, 0, 0)
    $p.SaveAs($Hasta, 24)
    $p.Close()
  } finally { $app.Quit() }
}
"#;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pixpin-office-convertido-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn se_convierten_los_word_y_powerpoint_de_antes() {
        for e in ["doc", "dot", "rtf", "odt"] {
            assert_eq!(destino(e), Some("docx"), "{e}");
        }
        for e in ["ppt", "pps", "pot", "odp"] {
            assert_eq!(destino(e), Some("pptx"), "{e}");
        }
        // Caso negativo: Excel lo lee el visor de hojas; Visio y los correos
        // no tienen visor propio; lo de ahora no hace falta.
        for e in ["xls", "ods", "vsd", "msg", "docx", "pptx", "pdf", ""] {
            assert_eq!(destino(e), None, "{e}");
        }
    }

    #[test]
    fn la_copia_cambia_si_cambia_el_original_y_se_lee_en_su_nombre() {
        let d = temporal("clave");
        let raiz = d.join("datos");
        let doc = d.join("Informe final.doc");
        std::fs::write(&doc, "uno").unwrap();
        let c1 = copia_de(&raiz, &doc).unwrap();
        assert_eq!(c1.file_name().unwrap(), "Informe final.docx");
        assert!(c1.starts_with(raiz.join("cache").join("office")));
        assert_eq!(copia_de(&raiz, &doc), Some(c1.clone()), "lo mismo, la misma copia");
        std::fs::write(&doc, "otro mas largo").unwrap();
        assert_ne!(copia_de(&raiz, &doc).unwrap(), c1, "cambio el original: otra copia");
        // Lista solo cuando esta; y entonces se lee en nombre del original.
        let c2 = copia_de(&raiz, &doc).unwrap();
        assert_eq!(lista(&raiz, &doc), None);
        std::fs::create_dir_all(c2.parent().unwrap()).unwrap();
        std::fs::write(&c2, "docx").unwrap();
        assert_eq!(lista(&raiz, &doc), Some(c2.clone()));
        assert_eq!(pixpin_proyecto::anotado::original_de(&c2), Some(doc.clone()));
        // Caso negativo: un Excel viejo no tiene copia.
        let xls = d.join("libro.xls");
        std::fs::write(&xls, "x").unwrap();
        assert_eq!(copia_de(&raiz, &xls), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Con Word de verdad: se crea un `.doc` con Word y se convierte.
    #[test]
    #[ignore = "necesita Microsoft Word"]
    fn convierte_un_doc_con_word() {
        let d = temporal("word");
        let doc = d.join("prueba.doc");
        let guion = d.join("crear.ps1");
        std::fs::write(&guion, "param($r) $w = New-Object -ComObject Word.Application; try { $x = $w.Documents.Add(); $x.Content.Text = 'Hola PixPin'; $x.SaveAs2($r, 0); $x.Close(0) } finally { $w.Quit() }").unwrap();
        let ok = std::process::Command::new("powershell.exe").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(&guion).arg(&doc).status().unwrap();
        assert!(ok.success() && doc.is_file());
        let desde = Instant::now();
        let copia = convertir(&d.join("datos"), &doc).unwrap();
        eprintln!("convertido en {} ms", desde.elapsed().as_millis());
        let texto = pixpin_docs::abrir(&copia).expect("el .docx se lee");
        assert!(format!("{texto:?}").contains("Hola PixPin"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Con PowerPoint de verdad: un `.ppt` creado con el y convertido.
    #[test]
    #[ignore = "necesita Microsoft PowerPoint"]
    fn convierte_un_ppt_con_powerpoint() {
        let d = temporal("powerpoint");
        let ppt = d.join("charla.ppt");
        let guion = d.join("crear.ps1");
        std::fs::write(&guion, "param($r) $a = New-Object -ComObject PowerPoint.Application; try { $p = $a.Presentations.Add(0); $s = $p.Slides.Add(1, 1); $s.Shapes.Item(1).TextFrame.TextRange.Text = 'Hola PixPin'; $p.SaveAs($r, 1); $p.Close() } finally { $a.Quit() }").unwrap();
        let ok = std::process::Command::new("powershell.exe").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(&guion).arg(&ppt).status().unwrap();
        assert!(ok.success() && ppt.is_file());
        let copia = convertir(&d.join("datos"), &ppt).unwrap();
        assert_eq!(copia.extension().unwrap(), "pptx");
        assert!(std::fs::read(&copia).unwrap().starts_with(b"PK"), "un .pptx es un zip");
        let _ = std::fs::remove_dir_all(&d);
    }
}
