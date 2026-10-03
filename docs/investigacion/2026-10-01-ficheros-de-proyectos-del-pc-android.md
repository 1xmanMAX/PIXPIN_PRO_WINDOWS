# Ficheros con `)` en el nombre: no entran en lo que se sincroniza (Android)

Fecha: 2026-10-01. Queja del usuario: un proyecto creado en el PC («TESIS RESERCH») con
dos `.md` llega al movil, pero al pulsar cada mensaje sale «Este archivo ya no está»
(`guardados_ya_no_esta`).

## Causa exacta

Los dos ficheros se llaman `Objetivos e Indicadores — Versión para la segunda asesoría (1).md`
y `(2).md`. `Disco.alcance` (lo que cada aparato dice tener de un chat) saca las rutas de
cada mensaje con `Rutas.enTexto`, y `enTexto` corta la ruta en el primer caracter de `FIN`:

```kotlin
// app/src/main/java/com/forge/pixpin/sincro/Disco.kt, linea 892
private val FIN = setOf('"', ')', '\\', '\n', '<', '>', '\'')
```

El `)` esta ahi para no comerse el cierre de un enlace de Markdown
(`(pixpin:files/x.png)`), pero corta tambien el nombre del fichero: `… asesoría (1).md` se
queda en `… asesoría (1`, que no existe, y `poner` lo descarta (`if (!f.isFile) return`).
El fichero nunca entra en la lista, nadie lo pide y no viaja; el mensaje si viaja, con su
`ruta` entera, y apunta a un fichero que el movil no tiene.

El PC tenia el mismo fallo (su `alcance_de` es el puerto de este codigo) y es el que
dejaba el `.md` en tierra: la base acordada con el movil para ese chat tiene
`"archivos": {}`. **Ya esta arreglado en el PC** (`DiscoPc::alcance` en
`crates/pixpin-proyecto/src/vista.rs` anade la `ruta` exacta de cada mensaje y el
`pdfOrigen`/`pdfLimpio` del proyecto). En la proxima vuelta el PC manda los dos `.md` y el
movil los abre.

Pasa con cualquier nombre con parentesis, que es lo mas comun: el propio PC pone « (1)» al
soltar dos veces un fichero con el mismo nombre, y Windows y Android hacen lo mismo en
Descargas.

## Lo que falta en Android

Sin esto el movil tampoco ve **sus** ficheros con `)` en el nombre: no los lista, y
1. lo que el movil adjunta con `)` en el nombre no llega nunca al PC;
2. lo que le llega del PC con `)` no figura en su lista, asi que en **cada vuelta** el PC
   se lo vuelve a mandar entero (no borra nada, pero gasta tiempo y datos y la vuelta
   nunca queda «quieta»). La prueba del PC
   `los_md_con_parentesis_de_un_proyecto_del_pc_se_abren_en_el_movil`
   (`crates/pixpin-proyecto/tests/sincronizar_con_el_movil.rs`) lo deja escrito.

### Cambio 1: la ruta exacta de cada mensaje

`app/src/main/java/com/forge/pixpin/sincro/Disco.kt`, en `fun alcance`, justo despues de
la linea 507:

```kotlin
for (rel in rutas.enTexto(JSON.encodeToString(Mensaje.serializer(), m))) poner(rel, etiqueta)
```

anadir:

```kotlin
// La ruta del adjunto, ENTERA: `enTexto` corta en `)` (por los enlaces de Markdown) y
// «informe (1).pdf» se quedaba en «informe (1», que no es un fichero (1-oct-2026).
m.ruta?.let { r -> (rutas.relativa(r) ?: r.removePrefix(Rutas.PORTATIL).takeIf { r.startsWith(Rutas.PORTATIL) })?.let { poner(it, etiqueta) } }
```

(`Rutas.PORTATIL` es la constante de la linea 891; si `relativa` ya cubre todos los casos
porque los mensajes se guardan con la ruta absoluta, basta con
`m.ruta?.let { rutas.relativa(it) }?.let { poner(it, etiqueta) }`.)

### Cambio 2: el documento del proyecto

En el mismo `fun alcance`, despues de la linea 526:

```kotlin
for (rel in rutas.enTexto(Proyectos.json.encodeToString(Proyecto.serializer(), p))) poner(rel, p.nombre)
```

anadir:

```kotlin
for (r in listOfNotNull(p.pdfOrigen, p.pdfLimpio)) rutas.relativa(r)?.let { poner(it, p.nombre) }
```

### No cambiar

- `FIN` y `enTexto` se quedan como estan: siguen haciendo falta para las rutas que van
  dentro de un texto (una nota en Markdown, un lienzo).
- `poner` ya descarta lo repetido (`rel in salida`) y lo que no existe, asi que la ruta
  buena se suma sin duplicar y la cortada sigue sin entrar.

### Como comprobarlo

1. En el movil, adjuntar al chat un fichero llamado `prueba (1).txt` y sincronizar con el
   PC: tiene que abrirse en el PC.
2. Sincronizar dos veces seguidas con un proyecto del PC que tenga un `… (1).md`: la
   segunda vuelta no debe mandar ningun archivo.
