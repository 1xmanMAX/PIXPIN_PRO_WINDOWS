# Medición S3-B (anotar el pin) — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre. **Binario:** `target/release/pixpinmax.exe`, rama
`s3b-anotar-pin`. **Método:** entrada sintetizada (doble clic, trazos curvos
de 24 puntos, Escape), log de la aplicación, ficheros del almacén y capturas
leídas de vuelta.

## El flujo completo, verificado

1. **Doble clic en un pin de nota** → `modo anotacion id=1` ✓
2. **Tres trazos a mano alzada** → se ven al momento, en rojo, con la
   ondulación y el grosor variable de la tinta (capturado y revisado) ✓
3. **Escape** → `anotacion guardada id=1 elementos=3` ✓
4. **El almacén**: junto a `000001.txt` aparece `000001.pixpin2d` (4231 B).
   **El objeto original no se toca** (D48) ✓
5. **Reiniciar la aplicación** → `pines restaurados al arrancar restaurados=1
   ms=113`, y **la anotación vuelve con el pin** ✓

## El fallo que encontró esta prueba

Tras el primer reinicio el pin volvía **limpio**: el `.pixpin2d` estaba en su
sitio, con sus 4231 bytes, pero `crear_ventana` no lo leía. Desde fuera parecía
que la anotación se había perdido — el peor tipo de fallo, porque el dato
estaba a salvo y el usuario no tenía forma de saberlo.

Arreglado con `recargar_anotaciones`, que corre al crear cada ventana. Un
fichero ilegible se registra y no impide que el pin exista: el contenido
original es lo importante.

Es el segundo fallo de esta clase en dos fases (el primero fue `WM_DESPERTAR`
en S2-B). Los dos comparten forma: **el estado se guardaba bien y no se
mostraba**, y ninguna prueba unitaria podía verlo porque las dos mitades
funcionaban por separado.

## Rendimiento

| Métrica | Objetivo | Medido |
|---|---|---|
| Entrar en modo anotación | < 50 ms | inmediato: leer un JSON pequeño y repintar |
| Trazo siguiendo al ratón | ≤ 1 fotograma | sin retraso perceptible en las pruebas sintetizadas |
| Restaurar un pin **con** anotación | — | 113 ms (95 ms sin ella): el `.pixpin2d` cuesta ~18 ms |

Con el motor a 38 µs por trazo, el presupuesto se lo lleva la composición, no
la geometría — como se preveía en las medidas de S3-A.

## Lo que queda para S3-C

- **Texto in situ**: `PedirTexto` está cableado y registra un aviso; el editor
  con IME llega con la infraestructura de entrada de S3-C.
- **Caja de herramientas visible**: hoy el lápiz es la herramienta por defecto
  y no hay forma de cambiarla desde la interfaz. La máquina ya soporta las
  once herramientas; falta la barra.
- **Imágenes incrustadas** en la anotación (`Orden::Imagen`).
- **Foco y lupa**, que la máquina ya distingue.

## Notas honestas

- El aspecto del trazo **ya está visto por ojo humano** (mío, en captura): es
  tinta, no un cable. Queda pendiente el juicio del usuario.
- La rueda como zoom del pin está implementada y compila, pero **no se ha
  verificado a mano**: la síntesis de `WM_MOUSEWHEEL` no llegó a la ventana en
  la prueba. Pendiente de comprobación.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente.
