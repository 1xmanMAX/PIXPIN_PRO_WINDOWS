"""Corre los modelos de Indra con onnxruntime para el ejemplo de paridad en Rust.

Protocolo por stdin/stdout: una linea JSON {"m": modelo, "entradas": [[nombre, dtype, forma], ...]}
seguida de los bytes de cada entrada; responde una linea JSON {"salidas": [[nombre, dtype, forma], ...]}
seguida de los bytes. Uso: python -I servidor_ort.py <carpeta_modelos> <hilos>
"""
import json, sys
import numpy as np
import onnxruntime as ort

d, hilos = sys.argv[1], int(sys.argv[2])
so = ort.SessionOptions()
so.intra_op_num_threads = hilos
so.inter_op_num_threads = 1
so.enable_cpu_mem_arena = False
ses = {m: ort.InferenceSession(f"{d}/{m}.onnx", so, providers=["CPUExecutionProvider"]) for m in ["det", "rec", "order", "orient"]}
ent, sal = sys.stdin.buffer, sys.stdout.buffer
while True:
    linea = ent.readline()
    if not linea:
        break
    pet = json.loads(linea)
    feed = {}
    for nombre, dt, forma in pet["entradas"]:
        n = int(np.prod(forma)) * np.dtype(dt).itemsize
        feed[nombre] = np.frombuffer(ent.read(n), dtype=dt).reshape(forma)
    s = ses[pet["m"]]
    nombres = [o.name for o in s.get_outputs()]
    outs = s.run(nombres, feed)
    cab = [[n, str(o.dtype), list(o.shape)] for n, o in zip(nombres, outs)]
    sal.write((json.dumps({"salidas": cab}) + "\n").encode())
    for o in outs:
        sal.write(np.ascontiguousarray(o).tobytes())
    sal.flush()
