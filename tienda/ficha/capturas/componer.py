#!/usr/bin/env python3
"""Compone las capturas de la Microsoft Store (1920×1080) a partir de las
muestras que pintan las pruebas `#[ignore]` de `apps/pindesk` (Direct2D fuera
de pantalla, ver `.github/workflows/capturas-tienda.yml`).

Cada escena toma la primera muestra que exista de su lista, la centra sobre
un fondo suave con sombra y esquinas redondeadas, y le pone encima un titulo
corto en cada idioma. Una escena sin muestra no para nada: se apunta en el
informe.

Uso:
    python componer.py <carpeta-con-las-muestras> <carpeta-de-salida>

Deja `<salida>/<idioma>/NN-<escena>.png` y `<salida>/informe.md`.
Necesita Pillow.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ANCHO, ALTO = 1920, 1080

# (escena, muestras posibles en orden de preferencia, {idioma: (titulo, subtitulo)})
ESCENAS = [
    (
        "chat",
        ["foto-con-descripcion.png", "chat-con-la-zona.png"],
        {
            "es": ("Cada proyecto es un chat", "Notas, fotos con descripción, voz, documentos y lienzos, juntos"),
            "en": ("Every project is a chat", "Notes, captioned photos, voice, documents and canvases together"),
        },
    ),
    (
        "captura",
        ["captura2-despues.png", "captura2-elegir.png"],
        {
            "es": ("Captura y anota al instante", "Zona, ventana, pantalla o scroll; copia, pinea o mándalo al chat"),
            "en": ("Capture and annotate instantly", "Region, window, full screen or scrolling; copy, pin or send to a chat"),
        },
    ),
    (
        "asistente",
        ["chat-asistente-burbujas.png"],
        {
            "es": ("Un asistente que trabaja en tus proyectos", "Opcional, con tu propia clave de Claude o DeepSeek"),
            "en": ("An assistant that works in your projects", "Optional, with your own Claude or DeepSeek key"),
        },
    ),
    (
        "tareas",
        ["tareas-kanban.png", "tareas-v5-tarjetas.png", "tareas-v5-lista.png"],
        {
            "es": ("Tareas en lista o en Kanban", "Escríbelas como hablas: «#casa llamar al banco 18/10 17:00»"),
            "en": ("Tasks as a list or a Kanban board", "Type them the way you talk, with dates, lists and priorities"),
        },
    ),
    (
        "horario",
        ["horario-semanal.png"],
        {
            "es": ("Tu horario de clases, con avisos", "Y, si quieres, tus eventos de Google Calendar al lado"),
            "en": ("Your class schedule, with reminders", "Optionally next to your Google Calendar events"),
        },
    ),
    (
        "lienzo",
        ["grafica-ecuaciones-y-tabla.png", "lienzo-en-ventana.png", "zona-papel-claro.png"],
        {
            "es": ("Un lienzo infinito", "Figuras, tinta, cotas, gráficas y tablas; exporta a PNG, SVG o PDF"),
            "en": ("An infinite canvas", "Shapes, ink, dimensions, graphs and tables; export to PNG, SVG or PDF"),
        },
    ),
    (
        "timeline",
        ["timeline-estado.png", "timeline-hoy.png", "timeline-momentos.png"],
        {
            "es": ("Tu día, minuto a minuto", "Una línea de tiempo y un registro de lecciones aprendidas"),
            "en": ("Your day, minute by minute", "A timeline and a log of lessons learned"),
        },
    ),
    (
        "buscar",
        ["buscar2-resultados.png", "buscar2-resultados-dia.png"],
        {
            "es": ("Busca en todo", "También en el texto que hay dentro de tus capturas"),
            "en": ("Search everything", "Including the text inside your screenshots"),
        },
    ),
    (
        "ajustes",
        ["ajustes2-asistente.png", "ajustes2-captura.png"],
        {
            "es": ("Tú decides qué se conecta", "Sin cuentas, sin nube propia y sin telemetría"),
            "en": ("You decide what connects", "No account, no PinDesk cloud and no telemetry"),
        },
    ),
]

# Fondo: azul del icono hacia un gris azulado muy claro.
ARRIBA = (232, 240, 252)
ABAJO = (205, 222, 247)
TINTA = (24, 33, 48)
TINTA_SUAVE = (74, 88, 110)
AZUL = (42, 122, 226)


def letra(tam: int, negrita: bool = False) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    """Segoe UI en Windows; si no, DejaVu o la de Pillow."""
    windir = os.environ.get("WINDIR", r"C:\Windows")
    candidatas = (
        [f"{windir}\\Fonts\\seguisb.ttf", f"{windir}\\Fonts\\segoeuib.ttf"] if negrita else []
    ) + [
        f"{windir}\\Fonts\\segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf" if negrita else "",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    for c in candidatas:
        if c and Path(c).exists():
            return ImageFont.truetype(c, tam)
    return ImageFont.load_default(tam)


def fondo() -> Image.Image:
    img = Image.new("RGB", (ANCHO, ALTO), ARRIBA)
    d = ImageDraw.Draw(img)
    for y in range(ALTO):
        t = y / (ALTO - 1)
        d.line([(0, y), (ANCHO, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(ARRIBA, ABAJO)))
    # Un circulo grande y muy suave del color del icono, abajo a la derecha.
    capa = Image.new("RGBA", (ANCHO, ALTO), (0, 0, 0, 0))
    ImageDraw.Draw(capa).ellipse((ANCHO - 700, ALTO - 520, ANCHO + 500, ALTO + 680), fill=AZUL + (28,))
    img.paste(capa, (0, 0), capa)
    return img


def redondear(img: Image.Image, radio: int) -> Image.Image:
    mascara = Image.new("L", img.size, 0)
    ImageDraw.Draw(mascara).rounded_rectangle((0, 0, img.width - 1, img.height - 1), radio, fill=255)
    out = img.convert("RGBA")
    out.putalpha(mascara)
    return out


def componer(muestra: Path, titulo: str, subtitulo: str) -> Image.Image:
    lienzo = fondo()
    d = ImageDraw.Draw(lienzo)
    f_titulo, f_sub = letra(64, True), letra(32)
    d.text((ANCHO // 2, 92), titulo, font=f_titulo, fill=TINTA, anchor="mm")
    d.text((ANCHO // 2, 160), subtitulo, font=f_sub, fill=TINTA_SUAVE, anchor="mm")

    app = Image.open(muestra).convert("RGBA")
    # Fondo blanco bajo las transparencias (las muestras con alfa).
    base = Image.new("RGBA", app.size, (255, 255, 255, 255))
    app = Image.alpha_composite(base, app)
    caja_w, caja_h = 1680, 840
    escala = min(caja_w / app.width, caja_h / app.height, 1.6)
    tam = (max(1, round(app.width * escala)), max(1, round(app.height * escala)))
    app = app.resize(tam, Image.LANCZOS)
    app = redondear(app, 18)
    x = (ANCHO - app.width) // 2
    y = 210 + (caja_h - app.height) // 2
    sombra = Image.new("RGBA", (ANCHO, ALTO), (0, 0, 0, 0))
    ImageDraw.Draw(sombra).rounded_rectangle(
        (x, y + 14, x + app.width, y + app.height + 14), 22, fill=(20, 40, 80, 70)
    )
    sombra = sombra.filter(ImageFilter.GaussianBlur(22))
    lienzo = Image.alpha_composite(lienzo.convert("RGBA"), sombra)
    lienzo.alpha_composite(app, (x, y))
    return lienzo.convert("RGB")


def indice(carpeta: Path) -> dict[str, Path]:
    """Nombre -> ruta de todas las PNG bajo la carpeta (la mas reciente gana)."""
    vistos: dict[str, Path] = {}
    for p in sorted(carpeta.rglob("*.png"), key=lambda p: p.stat().st_mtime):
        vistos[p.name] = p
    return vistos


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    entrada, salida = Path(sys.argv[1]), Path(sys.argv[2])
    salida.mkdir(parents=True, exist_ok=True)
    muestras = indice(entrada)
    informe = ["# Capturas para la Store", "", f"Muestras encontradas: {len(muestras)}", ""]
    informe += ["| # | Escena | Muestra usada | Tamaño original |", "|---|---|---|---|"]
    n = 0
    for escena, posibles, textos in ESCENAS:
        elegida = next((muestras[m] for m in posibles if m in muestras), None)
        if elegida is None:
            informe.append(f"| – | {escena} | **falta** (buscaba {', '.join(posibles)}) | – |")
            continue
        n += 1
        with Image.open(elegida) as im:
            tam = f"{im.width}×{im.height}"
        for idioma, (titulo, sub) in textos.items():
            destino = salida / idioma / f"{n:02d}-{escena}.png"
            destino.parent.mkdir(parents=True, exist_ok=True)
            componer(elegida, titulo, sub).save(destino, optimize=True)
        informe.append(f"| {n:02d} | {escena} | `{elegida.name}` | {tam} |")
    informe += ["", "Todas las muestras:", ""] + [f"- `{k}`" for k in sorted(muestras)]
    (salida / "informe.md").write_text("\n".join(informe) + "\n", encoding="utf-8")
    print("\n".join(informe[:14 + len(ESCENAS)]))
    if n == 0:
        print("::warning::ninguna escena tenia muestra")
    return 0


if __name__ == "__main__":
    sys.exit(main())
