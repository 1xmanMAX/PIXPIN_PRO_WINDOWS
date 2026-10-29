#!/usr/bin/env bash
# **De punta a punta** (docs/probar-ideas-sin-interfaz.md): un guion de
# reunion se lee en voz alta con voces sinteticas (Piper, una voz por
# persona, con un poco de ruido de sala y eco), el audio pasa por la escucha
# en vivo de la app (`pixpin-voz` `frases_en_vivo`: `Frasero`, `Cola`,
# `Freno` y Whisper base, en tiempo real simulado) y lo que oye Whisper,
# sin quien habla («Voz») y con la hora en que se dejo de hablar y la del
# texto, pasa por los prototipos de Jev (`temas_en_vivo` y
# `reunion_completa`) con las esperas del guion.
#
#   crates/pixpin-typesafe/examples/guiones/de_punta_a_punta.sh audio_coordinacion.txt
#
# Hace falta (nada de esto va al repositorio):
#   PIXPIN_VOZ   carpeta con raiz/whisper/base/ y ort/libonnxruntime.so
#                (ver «Audio (pixpin-voz)» en el documento)
#   VOCES        carpeta con las voces de Piper (*.onnx y *.onnx.json)
#   PYTHON       un python con `piper-tts` (por omision, python3)
#   y la clave de Jev en TYPESAFE_API_KEY o TYPESAFE_API_KEY_FILE.
# Opcional:
#   SALIDA       donde dejar el audio y lo oido (por omision, una carpeta
#                temporal nueva)
#   QUE          que pasos: «hablar oir jev limpio» (por omision, todos;
#                «limpio» corre tambien el guion escrito, con y sin nombres)
#   RUIDO        amplitud del ruido rosa (0.004; 0 sin ruido)
#   LISTA_VOCES  las voces por orden de aparicion de cada persona
#                («voz» o «voz:hablante»)
#   JEV          opciones de mas para los prototipos (cada uno graba sus
#                respuestas en $SALIDA/<guion>.<ejemplo>.<cual>.jsonl)
set -euo pipefail

guion=${1:?uso: de_punta_a_punta.sh GUION}
raiz_repo=$(cd "$(dirname "$0")/../../../.." && pwd)
[[ -f $guion ]] || guion=$raiz_repo/crates/pixpin-typesafe/examples/guiones/$guion
nombre=$(basename "$guion" .txt)
: "${PIXPIN_VOZ:?falta PIXPIN_VOZ}" "${VOCES:?falta VOCES}"
PYTHON=${PYTHON:-python3}
SALIDA=${SALIDA:-$(mktemp -d)}
QUE=${QUE:-hablar oir jev limpio}
RUIDO=${RUIDO:-0.004}
LISTA_VOCES=${LISTA_VOCES:-es_MX-claude-high es_ES-davefx-medium es_ES-sharvard-medium:1 es_ES-sharvard-medium:0}
mkdir -p "$SALIDA"
d=$SALIDA/$nombre
cd "$raiz_repo"
hay() { [[ " $QUE " == *" $1 "* ]]; }

if hay hablar; then
  echo "━━ hablar: $guion → $d.wav"
  # Cada linea con la voz de quien la dice; entre una y otra, un silencio
  # de 0,25-0,8 s (fijo por linea); un «—» delante pisa la anterior 0,35 s.
  # En $d.verdad.tsv, donde empieza y acaba de verdad cada linea.
  "$PYTHON" -I - "$guion" "$VOCES" "$d" $LISTA_VOCES <<'PY'
import random, re, sys, wave
from array import array
from piper import PiperVoice, SynthesisConfig

guion, voces, salida, lista = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
lineas = []
for l in open(guion, encoding="utf-8"):
    m = re.match(r"\s*\[[^\]]+\]\s*([^:]+):\s*(.+)", l)
    if m:
        lineas.append((m.group(1).strip(), m.group(2).strip()))
cargadas, de_quien = {}, {}
for quien, _ in lineas:
    if quien not in de_quien:
        de_quien[quien] = lista[len(de_quien) % len(lista)]
hz = None
pista = array("h")
verdad = []
fin = 0
for n, (quien, dice) in enumerate(lineas):
    voz, _, hablante = de_quien[quien].partition(":")
    if voz not in cargadas:
        cargadas[voz] = PiperVoice.load(f"{voces}/{voz}.onnx")
    v = cargadas[voz]
    hz = hz or v.config.sample_rate
    assert v.config.sample_rate == hz, "todas las voces al mismo muestreo"
    encima = dice.startswith("—")
    texto = dice.lstrip("— ").strip()
    cfg = SynthesisConfig(speaker_id=int(hablante) if hablante else None)
    muestras = array("h")
    for trozo in v.synthesize(texto, syn_config=cfg):
        muestras.frombytes(trozo.audio_int16_bytes)
    r = random.Random(n)
    hueco = -0.35 if encima else r.uniform(0.25, 0.8)
    empieza = max(0, fin + int(hueco * hz)) if n else int(0.5 * hz)
    falta = empieza + len(muestras) - len(pista)
    if falta > 0:
        pista.extend([0] * falta)
    for i, m in enumerate(muestras):
        pista[empieza + i] = max(-32768, min(32767, pista[empieza + i] + m))
    fin = max(fin, empieza + len(muestras))
    verdad.append((empieza / hz, (empieza + len(muestras)) / hz, quien, de_quien[quien], texto))
pista.extend([0] * int(1.5 * hz))
with wave.open(salida + ".limpio.wav", "wb") as w:
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(hz)
    w.writeframes(pista.tobytes())
with open(salida + ".verdad.tsv", "w", encoding="utf-8") as f:
    for e, a, q, v, t in verdad:
        f.write(f"{e:.2f}\t{a:.2f}\t{q}\t{v}\t{t}\n")
print(f"   {len(lineas)} lineas, {len(pista) / hz:.1f} s, voces: {de_quien}")
PY
  # Sala: un eco corto y ruido rosa de fondo, a 16 kHz mono.
  ffmpeg -loglevel error -y -i "$d.limpio.wav" \
    -f lavfi -i "anoisesrc=color=pink:amplitude=$RUIDO:seed=7" \
    -filter_complex "[0:a]aecho=0.8:0.5:30|55:0.18|0.10[e];[e][1:a]amix=inputs=2:duration=first:normalize=0[m]" \
    -map "[m]" -ac 1 -ar 16000 -c:a pcm_s16le "$d.wav"
fi

if hay oir; then
  echo "━━ oir: $d.wav → $d.oido.txt"
  CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0} cargo run -q -j 2 --release -p pixpin-voz \
    --example frases_en_vivo -- "$d.wav" --raiz "$PIXPIN_VOZ/raiz" --ort "$PIXPIN_VOZ/ort" \
    --rapido --salida "$d.frases.txt" | tee "$d.oido.log"
fi

jev() {
  local ej=$1 g=$2 sufijo=$3
  CARGO_INCREMENTAL=${CARGO_INCREMENTAL:-0} cargo run -q -j 2 -p pixpin-typesafe \
    --example "$ej" -- "$g" --rapido --grabar "$d.$ej.$sufijo.jsonl" ${JEV:-} \
    >"$d.$ej.$sufijo.log" || true
  grep -h '^RESUMEN\|^   ✗\|de que se deja de hablar' "$d.$ej.$sufijo.log" | sed "s/^/   [$sufijo] /"
}

if hay jev; then
  # El guion oido: la cabecera y las esperas del escrito, y las frases de
  # Whisper con sus horas.
  { grep '^#' "$guion"; echo; cat "$d.frases.txt"; } >"$d.oido.txt"
  echo "━━ jev: lo oido ($d.oido.txt)"
  jev temas_en_vivo "$d.oido.txt" oido
  jev reunion_completa "$d.oido.txt" oido
fi

if hay limpio; then
  echo "━━ jev: el guion escrito, con nombres y con «Voz»"
  sed -E 's/^(\[[^]]+\]) [^:]+:/\1 Voz:/' "$guion" >"$d.voz.txt"
  jev temas_en_vivo "$guion" escrito
  jev reunion_completa "$guion" escrito
  jev reunion_completa "$d.voz.txt" voz
fi
echo "━━ en $SALIDA"
