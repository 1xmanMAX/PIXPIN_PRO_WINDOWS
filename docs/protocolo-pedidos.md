# Pedidos a PixPin Max (protocolo v1)

Cualquier programa del mismo equipo puede pedirle cosas a la PixPin Max que esta abierta:
abrir un proyecto, escribir en el chat, crear un lienzo, grabar, añadir una tarea... Lo usa
el plugin de Flow Launcher (`apps/pixpin-lanzador`), y sirve igual para un script propio
(AutoHotkey, PowerShell, otro lanzador).

## Transporte

- `WM_COPYDATA` a la ventana de clase **`PixPinMaxVentanaMensajes`** (`FindWindowW`), con
  `dwData = 0x5049_5851` y en `lpData` el **JSON en UTF-8, sin nulo final**, de 64 KB como
  mucho. Se manda con `SendMessageW` (sincrono) y despues se le postea `WM_NULL` para
  despertar su bucle.
- Respuesta inmediata (`LRESULT`): `1` aceptado (en cola), `2` version no soportada, `3` JSON
  roto o accion desconocida, `0` no era para nosotros o no hay nadie escuchando.
- El trabajo de verdad lo hace el bucle principal de PixPin despues: lo que salga mal lo
  dice PixPin con su aviso, no la respuesta.
- Si no hay ventana, PixPin no esta abierta: se arranca
  `%LOCALAPPDATA%\Programs\PixPinMax\pixpinmax.exe` y se espera a la ventana (hasta 8 s).

## El pedido

```json
{ "pixpin": 1, "accion": "<accion>", ... }
```

- `pixpin` es la version. Un campo nuevo opcional no la sube; quitar o cambiar uno si.
- Lo que no se conoce se ignora.
- `proyecto` es el `id` de la carpeta del proyecto (`proyectos/<id>`, el de
  `proyectos/indice.json`). `null` o ausente = **Mensajes guardados**.
- `codigo` es el `id` del mensaje dentro de `guardados.jsonl`.

| accion | campos | que hace |
|---|---|---|
| `abrir` | `que: {tipo:"proyecto", proyecto}` | El chat en ese proyecto |
| `abrir` | `que: {tipo:"mensaje", proyecto, codigo}` | El chat en ese mensaje, resaltado |
| `abrir` | `que: {tipo:"hoja", proyecto, referencia}` | Ese lienzo u hoja, en el lienzo |
| `abrir` | `que: {tipo:"nota", proyecto, codigo}` | Esa nota en el editor de notas |
| `abrir` | `que: {tipo:"fichero", ruta}` | Como «Abrir con PixPin» (lector, editor, pin) |
| `chat` | `texto`, `proyecto?` | Un mensaje de texto nuevo en ese chat |
| `nota_nueva` | `texto?`, `proyecto?` | Una nota nueva, abierta en el editor |
| `lienzo_nuevo` | `nombre?`, `proyecto?` | Un lienzo nuevo, abierto |
| `grabar` | `nombre`, `proyecto?` | Saca el micrófono flotante y graba una nota de voz; un clic la guarda con ese nombre (`<nombre>.m4a`) en ese chat |
| `lista_nueva` | `titulo`, `proyecto?` | Una lista de tareas nueva (mini-app `tareas`) |
| `anadir_tarea` | `texto`, `proyecto?`, `codigo?` | Añade `- [ ] texto` a la lista `codigo` (sin `codigo`: la ultima lista de tareas de ese chat, o una nueva «Tareas») |
| `marcar_tarea` | `proyecto?`, `codigo`, `indice`, `hecha` | Marca (o desmarca) la tarea numero `indice` (desde 0, en el orden del documento) de la lista `codigo` |
| `ventana_principal` | — | Saca el chat |
| `pinear` | `ruta` o (`proyecto?`, `codigo`) | Saca ese fichero (o el del mensaje) a la pantalla como pin, como «Sacar a la pantalla» del chat |
| `iconos` | `extensiones: ["pdf","docx",...]` | Pinta el icono de archivo de cada extension (el del chat: hoja de su color con la extension escrita) en `<raiz>/cache/iconos-de-extension/<ext>.png` (64 px, fondo transparente) si aun no esta. No abre nada ni avisa |
| `soltar` | `proyecto?` | Saca una caja flotante (siempre encima) del proyecto: los ficheros que se le suelten entran en su chat como si se soltaran en la ventana del chat. Una por proyecto; pedirla otra vez la trae delante. Se cierra con su aspa o Esc |

`grabar` no abre el chat: saca un **microfono flotante** siempre encima, con la señal de que
graba y el tiempo; un clic lo para y lo guarda (con su `nombre`) en el chat pedido. Al
guardarse ofrece **«Convertir en llamada»** con un selector de hora (la llamada secreta de
PixPin) y desaparece al acabar (o sola a los 10 s). Esc mientras graba la descarta sin preguntar.

## Donde leer (sin pedir nada)

Buscar se hace leyendo el disco, sin pasar por la app (funciona aunque este cerrada). La raiz
es `%APPDATA%\PixPinMax` (o la carpeta del exe si al lado hay un `pixpinmax.toml`):

- `proyectos/indice.json`: `{"proyectos":[{id, nombre, tocado, hojas, guardados?, ...}]}`.
  El de `guardados: true` es «Mensajes guardados».
- `proyectos/<id>/guardados.jsonl`: un mensaje por linea (`id, cuando, clase, texto, nombre,
  ruta, referencia, miniapp, transcripcion, enBuzon...`). `clase`: `NOTA, IMAGEN, ARCHIVO,
  VOZ, DIBUJO, PAGINA, PROYECTO, MINIAPP`. Las listas de tareas son `MINIAPP` con
  `miniapp:"tareas"` y el documento en `texto` (`# Titulo` y casillas `- [ ] x` / `- [x] x`).
- `proyectos/<id>/proyecto.json`: las hojas del proyecto.
- Nunca escribir en esos ficheros desde fuera: la app los protege con un cerrojo que solo
  existe dentro de ella. Para cambiar algo, un pedido.

## Desde la linea de mandatos

`pixpin-lanzador.exe pedido '<json>'` manda un pedido (arranca PixPin si hace falta) y
`pixpin-lanzador.exe buscar "<texto>"` escribe los resultados, una linea JSON por resultado.
Sin argumentos es el plugin de Flow Launcher (JSON-RPC por stdin/stdout).
