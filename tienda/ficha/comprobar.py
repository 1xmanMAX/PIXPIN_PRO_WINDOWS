#!/usr/bin/env python3
"""Comprueba las fichas de la Microsoft Store (`tienda/ficha/<idioma>.md`).

Cada ficha tiene un apartado `## <campo>` por campo de Partner Center y, dentro,
un bloque ```text con lo que se pega tal cual. Este script saca cada bloque y
mira los limites de Partner Center (los de la pagina «Store listing»):

- Product name: «PinDesk», igual en todos los idiomas (el nombre reservado).
- Description: obligatoria, 10 000 caracteres como mucho.
- What's new in this version: 1500 como mucho.
- Product features: hasta 20, una por renglon, 200 caracteres cada una.
- Short description: 1000 como mucho.
- Search terms: hasta 7, 30 caracteres cada uno y 21 palabras entre todos.
- Copyright and trademark info: 200 como mucho.
- Additional license terms: 10 000 como mucho.
- Support contact info, Website y Privacy policy URL: direcciones https.

Ademas: que no salga la marca «PixPin» (es de DepthPixel) ni restos sin
traducir conocidos, y que estan los 14 idiomas de la interfaz.

Uso: `python3 tienda/ficha/comprobar.py` (sale con 1 si algo falla).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CARPETA = Path(__file__).resolve().parent

IDIOMAS = [
    "es-ES", "en-US", "zh-CN", "ja-JP", "ko-KR", "hi-IN", "ar-SA",
    "tr-TR", "pt-BR", "it-IT", "fr-FR", "de-DE", "ru-RU", "id-ID",
]

NOMBRE = "PinDesk"
REPO = "https://github.com/1xmanMAX/PIXPIN_PRO_WINDOWS"
PRIVACIDAD = REPO + "/blob/main/docs/privacidad.md"

# campo -> (obligatorio, maximo de caracteres del bloque entero)
CAMPOS = {
    "Product name": (True, 256),
    "Description": (True, 10_000),
    "What's new in this version": (False, 1_500),
    "Product features": (False, None),
    "Short description": (False, 1_000),
    "Search terms": (False, None),
    "Copyright and trademark info": (False, 200),
    "Additional license terms": (False, 10_000),
    "Support contact info": (False, 2_048),
    "Website": (False, 2_048),
    "Privacy policy URL": (False, 2_048),
}

BLOQUE = re.compile(r"```text\n(.*?)\n```", re.S)


def campos_de(texto: str) -> dict[str, str]:
    """Cada `## campo` con el contenido de su primer bloque ```text."""
    salida: dict[str, str] = {}
    partes = re.split(r"^## +(.+?)\s*$", texto, flags=re.M)
    for i in range(1, len(partes), 2):
        titulo = partes[i].split(" · ")[0].strip()
        b = BLOQUE.search(partes[i + 1])
        if b:
            salida[titulo] = b.group(1)
    return salida


def comprobar(idioma: str) -> tuple[list[str], dict[str, int]]:
    errores: list[str] = []
    cuentas: dict[str, int] = {}
    ruta = CARPETA / f"{idioma}.md"
    if not ruta.exists():
        return [f"falta {ruta.name}"], cuentas
    c = campos_de(ruta.read_text(encoding="utf-8"))
    for campo, (obligatorio, maximo) in CAMPOS.items():
        v = c.get(campo)
        if v is None:
            if obligatorio or campo in ("Search terms", "Product features"):
                errores.append(f"falta el campo «{campo}»")
            continue
        cuentas[campo] = len(v)
        if not v.strip():
            errores.append(f"«{campo}» vacio")
        if maximo is not None and len(v) > maximo:
            errores.append(f"«{campo}»: {len(v)} caracteres (maximo {maximo})")
        if "PixPin" in v or "pixpin" in v.lower().replace("pixpin_pro_windows", ""):
            errores.append(f"«{campo}» nombra «PixPin» (marca de DepthPixel)")
        if re.search(r"\bTODO\b|\bTBD\b|XXX", v):
            errores.append(f"«{campo}» tiene un TODO")
    if c.get("Product name", "").strip() != NOMBRE:
        errores.append(f"el nombre no es «{NOMBRE}»")
    rasgos = [r for r in c.get("Product features", "").split("\n") if r.strip()]
    cuentas["features (n)"] = len(rasgos)
    if len(rasgos) > 20:
        errores.append(f"{len(rasgos)} features (maximo 20)")
    for r in rasgos:
        if len(r) > 200:
            errores.append(f"feature de {len(r)} caracteres (maximo 200): {r[:40]}…")
    terminos = [t.strip() for t in c.get("Search terms", "").split("\n") if t.strip()]
    palabras = sum(len(t.split()) for t in terminos)
    cuentas["terms (n)"] = len(terminos)
    cuentas["terms (palabras)"] = palabras
    if len(terminos) > 7:
        errores.append(f"{len(terminos)} search terms (maximo 7)")
    for t in terminos:
        if len(t) > 30:
            errores.append(f"search term de {len(t)} caracteres (maximo 30): {t}")
    if palabras > 21:
        errores.append(f"search terms con {palabras} palabras (maximo 21)")
    if len(set(t.lower() for t in terminos)) != len(terminos):
        errores.append("search terms repetidos")
    if "2026" not in c.get("Copyright and trademark info", "") or "Ming Xuan" not in c.get(
        "Copyright and trademark info", ""
    ):
        errores.append("el copyright no es «© 2026 Ming Xuan»")
    for campo in ("Support contact info", "Website", "Privacy policy URL"):
        v = c.get(campo, "").strip()
        if v and not v.startswith("https://"):
            errores.append(f"«{campo}» no es una direccion https")
    if c.get("Privacy policy URL", "").strip() != PRIVACIDAD:
        errores.append("la politica de privacidad no apunta a docs/privacidad.md de main")
    if not c.get("Website", "").strip().startswith(REPO):
        errores.append("la web no es la del repositorio")
    return errores, cuentas


def main() -> int:
    mal = 0
    print(f"{'idioma':7} {'desc':>6} {'nuevo':>6} {'corta':>6} {'feat':>4} {'term':>4} {'pal':>4}  estado")
    for idioma in IDIOMAS:
        errores, k = comprobar(idioma)
        estado = "bien" if not errores else "MAL"
        nuevo = k.get("What's new in this version", 0)
        print(
            f"{idioma:7} {k.get('Description', 0):>6} {nuevo:>6} "
            f"{k.get('Short description', 0):>6} {k.get('features (n)', 0):>4} "
            f"{k.get('terms (n)', 0):>4} {k.get('terms (palabras)', 0):>4}  {estado}"
        )
        for e in errores:
            print(f"    - {e}")
        mal += bool(errores)
    sobrantes = sorted(
        p.stem for p in CARPETA.glob("*-*.md") if p.stem not in IDIOMAS
    )
    if sobrantes:
        print("ficheros de idioma que no son de la interfaz:", ", ".join(sobrantes))
        mal += 1
    print("todo bien" if not mal else f"{mal} con fallos")
    return 1 if mal else 0


if __name__ == "__main__":
    sys.exit(main())
