"""Banco de Indra para PixPin: imagenes sinteticas en espanol + la muestra del repo.

Uso: python -I banco.py <carpeta_modelos> <carpeta_salida> <hilos> [scan.png]
"""
import json, os, random, resource, sys, time
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

modelos, salida, hilos = sys.argv[1], sys.argv[2], int(sys.argv[3])
extra = sys.argv[4] if len(sys.argv) > 4 else None
os.makedirs(salida, exist_ok=True)
F = "/usr/share/fonts/truetype/liberation/"

PARRAFO = (
    "La evaluación del análisis sísmico de la edificación se realizó con la norma vigente. "
    "Según el informe técnico, el desplazamiento máximo fue de 12,5 cm en el último piso. "
    "Además, la configuración estructural presenta irregularidades en planta y en altura. "
    "El código del proyecto es U20191A543 y la fecha de entrega, el 14 de marzo de 2026. "
    "Por último, se recomienda reforzar las columnas del eje B con encamisado de acero."
)


def envolver(texto, fuente, ancho, d):
    lineas, actual = [], ""
    for p in texto.split():
        prueba = (actual + " " + p).strip()
        if d.textlength(prueba, font=fuente) > ancho and actual:
            lineas.append(actual)
            actual = p
        else:
            actual = prueba
    if actual:
        lineas.append(actual)
    return lineas


def pantalla():
    img = Image.new("RGB", (1280, 720), (245, 246, 248))
    d = ImageDraw.Draw(img)
    d.rectangle((0, 0, 1280, 48), fill=(32, 33, 36))
    f = ImageFont.truetype(F + "LiberationSans-Regular.ttf", 15)
    fb = ImageFont.truetype(F + "LiberationSans-Bold.ttf", 22)
    verdad = []
    d.text((16, 13), "Configuración del sistema", font=ImageFont.truetype(F + "LiberationSans-Regular.ttf", 17), fill=(255, 255, 255))
    verdad.append("Configuración del sistema")
    d.text((40, 80), "Pantalla y brillo", font=fb, fill=(20, 20, 20))
    verdad.append("Pantalla y brillo")
    items = ["Resolución de pantalla: 1920 × 1080 (recomendado)", "Escala y diseño: 125 %",
             "Orientación: horizontal", "Luz nocturna: desactivada hasta las 21:00",
             "Varias pantallas: extender estas pantallas", "Contraseña de la red Wi-Fi: año2026ñandú",
             "Última actualización: miércoles, 22 de octubre de 2026"]
    y = 130
    for it in items:
        d.rounded_rectangle((32, y - 8, 900, y + 30), 6, fill=(255, 255, 255))
        d.text((48, y), it, font=f, fill=(30, 30, 30))
        verdad.append(it)
        y += 52
    # boton claro sobre oscuro
    d.rounded_rectangle((960, 640, 1240, 690), 8, fill=(0, 95, 184))
    d.text((990, 655), "Aplicar cambios", font=fb, fill=(255, 255, 255))
    verdad.append("Aplicar cambios")
    return img, verdad


def codigo():
    img = Image.new("RGB", (1600, 900), (30, 30, 30))
    d = ImageDraw.Draw(img)
    f = ImageFont.truetype(F + "LiberationMono-Regular.ttf", 14)
    lineas = ["fn reconocer(ancho: u32, alto: u32) -> Result<Vec<Linea>, ErrorOcr> {",
              "    // El motor se crea con los idiomas del perfil del usuario.",
              "    let motor = OcrEngine::TryCreateFromUserProfileLanguages()?;",
              "    let necesarios = (ancho as usize).checked_mul(alto as usize);",
              "    println!(\"líneas reconocidas: {}\", lineas.len());",
              "    if lineas.is_empty() { return Err(ErrorOcr::SinIdiomas); }",
              "    Ok(lineas)", "}"] * 4
    y = 20
    for l in lineas:
        d.text((20, y), l, font=f, fill=(212, 212, 212))
        y += 22
    return img, lineas


def pagina(escaneada=False):
    img = Image.new("RGB", (1240, 1754), (255, 255, 255))
    d = ImageDraw.Draw(img)
    ft = ImageFont.truetype(F + "LiberationSerif-Bold.ttf", 34)
    f = ImageFont.truetype(F + "LiberationSerif-Regular.ttf", 24)
    verdad = ["Informe de evaluación estructural"]
    d.text((120, 120), verdad[0], font=ft, fill=(0, 0, 0))
    y = 210
    for _ in range(3):
        for l in envolver(PARRAFO, f, 1000, d):
            d.text((120, y), l, font=f, fill=(0, 0, 0))
            verdad.append(l)
            y += 34
        y += 30
    if escaneada:
        g = np.asarray(img.convert("L")).astype(np.float32)
        h, w = g.shape
        sombra = np.linspace(1.0, 0.72, w)[None, :] * np.linspace(1.0, 0.9, h)[:, None]
        g = g * sombra * 0.93 + np.random.default_rng(1).normal(0, 9, g.shape)
        img = Image.fromarray(np.clip(g, 0, 255).astype(np.uint8)).rotate(1.8, resample=Image.BILINEAR, fillcolor=200)
        img = img.filter(ImageFilter.GaussianBlur(0.8)).resize((930, 1316), Image.BILINEAR)
    return img, verdad


def cer(ref, hip):
    a, b = ref, hip
    prev = list(range(len(b) + 1))
    for i in range(1, len(a) + 1):
        cur = [i] + [0] * len(b)
        for j in range(1, len(b) + 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] != b[j - 1]))
        prev = cur
    return prev[-1] / max(1, len(a))


def norm(lineas):
    return " ".join(" ".join(lineas).split())


casos = [("pantalla", *pantalla()), ("codigo", *codigo()), ("pagina", *pagina()), ("escaneada", *pagina(True))]
if extra:
    casos.append(("muestra_repo", Image.open(extra), None))

import indra_ocr

t0 = time.perf_counter()
conv = indra_ocr.load(path=modelos, threads=hilos, mode="ocr")
carga = time.perf_counter() - t0
rss_carga = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024
res = {"hilos": hilos, "carga_s": round(carga, 2), "rss_tras_carga_mb": round(rss_carga)}
for nombre, img, verdad in casos:
    p = os.path.join(salida, nombre + ".png")
    img.save(p)
    gray = np.asarray(img.convert("L"))
    conv.ocr(gray)  # calentamiento
    ts = []
    for _ in range(3):
        t = time.perf_counter()
        page = conv.ocr(gray)
        ts.append(time.perf_counter() - t)
    lineas = [l.text for l in page.lines]
    r = {"px": f"{gray.shape[1]}x{gray.shape[0]}", "lineas": len(lineas), "ms_mediana": round(1000 * sorted(ts)[1])}
    if verdad:
        r["cer_%"] = round(100 * cer(norm(verdad), norm(lineas)), 2)
    res[nombre] = r
    with open(os.path.join(salida, f"{nombre}_{hilos}h.txt"), "w") as fh:
        fh.write("\n".join(lineas))
res["rss_pico_mb"] = round(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024)
print(json.dumps(res, ensure_ascii=False))
