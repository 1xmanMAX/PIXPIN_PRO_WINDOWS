# Medición S2-B (tipos, grupos y portapapeles) — equipo de desarrollo — 2026-09-02

**Equipo:** el de siempre (16 GB, 4 núcleos físicos, Intel UHD + MX250, monitor
3000×2000 al 150 %). **Binario:** `target/release/pixpinmax.exe`, rama
`s2b-tipos-grupos`, modo portable. **Método:** entrada sintetizada, incluida la
navegación por teclado de los menús nativos; tiempos del log; estado leído de
`indice.json`; RAM/CPU con `Get-Process`.

## Las cinco puertas de la spec §7, ahora con los 10 pines que pedía

| Métrica | Objetivo | Medido |
|---|---|---|
| CPU con **10 pines** quietos | 0 % | **0,00 % — 0 ms de CPU en 10 s** ✓ |
| RAM privada con **10 pines** | < 100 MB (`Completo`) / < 60 MB (`Ligero`) | **18,1 MB** privados, 46,4 MB de working set ✓ con muchísimo margen |
| Arranque: **10 pines** restaurados | < 500 ms | **228 ms**, los diez visibles ✓ |
| Arranque: primer pin visible | < 200 ms | dentro de esos 228 ms (secuencial: el primero aparece mucho antes) ✓ |
| Mover un pin | ≤ 1 fotograma | `EfectoPin::Mover` sigue siendo solo `SetWindowPos`; sin repintado ✓ |

Los 10 pines eran notas de texto (~300×64 px lógicos cada una). Con imágenes
grandes la RAM subirá: la auditoría con capturas de pantalla completas queda
para cuando el vídeo y los documentos entren en S2-C.

**Consecuencia para el plan:** la Tarea 11 del plan de S2-B (restauración en
paralelo con presupuesto de texturas) **no se implementa**: 228 ms para diez
está a menos de la mitad del tope, y añadir hilos y reescalado a media
resolución sería complejidad sin problema que resolver. Queda anotada para
volver a ella si la auditoría con imágenes grandes de S2-C la justifica —con
los números delante, que es la regla de este proyecto.

## El flujo completo, comprobado punto a punto

1. **`Ctrl+Alt+V` con texto**: nota pineada, `.txt` UTF-8 legible con acentos y
   CJK intactos, sin robar el foco ✓
2. **`Ctrl+Alt+V` con dos archivos**: dos fichas por referencia; las rutas se
   anotan y **no se copia ni un byte** (D28) ✓
3. **Cascada**: pegar varias cosas ya no apila los pines en el mismo píxel ✓
4. **Menú del clic derecho**, traducido y con las entradas correctas por tipo:
   Copiar · Guardar como… · Tamaño original · Grupo ▸ · Cerrar · Eliminar del
   almacén… ✓
5. **Grupo → Verde**: el índice guarda `{"id":1,"color":"verde"}` y la sombra
   del pin se tiñe de verde al momento (verificado en captura) ✓
6. **Ocultar este grupo**: la ventana desaparece, el grupo queda `oculto:true`
   y **el rect se conserva** en el índice ✓
7. **Bandeja → Grupos ocultos → ● Verde (1)**: `grupo mostrado de nuevo
   id_grupo=1 vueltos=1`; el pin vuelve a su sitio exacto ✓

## El fallo que encontró esta prueba (y por eso se hace)

Las peticiones del menú (grupo, copiar, eliminar…) se encolaban correctamente
pero **nunca se atendían**: `Pines::purgar` solo corre cuando el bucle
principal procesa un evento de SU ventana, y el WndProc de un pin no produce
ninguno. El menú funcionaba, el clic se registraba, y no pasaba nada — sin
un solo error en el log.

Arreglo: `WM_DESPERTAR` (`WM_APP+2`), un mensaje vacío que el gestor publica en
la ventana principal tras encolar. El bucle da una vuelta y vacía la bandeja de
entrada. Ninguna prueba automática lo habría cazado: los tests unitarios no
tienen bucle de mensajes.

## Notas honestas

- La Duplicación de Escritorio siguió ocupada por **rustdesk** durante toda la
  sesión; la aplicación funcionó igual por WGC, que es exactamente el respaldo
  para el que se diseñó. Sus tres tests se saltan con aviso en esa situación.
- La automatización de menús nativos con el ratón resultó poco fiable (las
  coordenadas del proceso sin conciencia de DPI son las virtualizadas); con el
  teclado es determinista y así se hicieron todas las comprobaciones de menú.
- Pendiente de ojo humano: el aspecto fino de la nota y de la ficha (tipografía
  y espaciados), y el imán de bordes, que se probó en puro pero no a mano.
- El suelo real (i3 3.ª gen, 4 GB) sigue pendiente de tener la máquina delante.
