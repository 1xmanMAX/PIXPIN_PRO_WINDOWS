# Ventanas: donde estan y por donde se arrastran

## La regla: el origen de la ventana nunca se guarda para repartir clics

Los eventos de raton del overlay (`EventoOverlay::BotonPulsado(p)` y los
demas) llegan en coordenadas del **escritorio virtual**: el WndProc de
`pixpin-shell` suma al punto del cliente el origen que la ventana tiene **en
ese momento** (`GetWindowRect`). Cada ventana los pasa a las suyas restando
su `marco`.

Si ese `marco` es el de cuando se abrio, en cuanto otro mueve la ventana
—AltSnap (Alt + arrastrar), Win+flecha, el arrastre de Windows por la
cabecera— los botones se quedan donde estaba. Es lo que vio el usuario el
1-nov-2026: «muevo los ajustes con AltSnap y Capturas ya no se pulsa; si la
vuelvo a dejar donde estaba, funciona otra vez». AltSnap mueve con
`SetWindowPos`, sin `WM_ENTERSIZEMOVE` ni `WM_EXITSIZEMOVE`, asi que nada que
solo escuche el arrastre propio se entera.

Por eso:

- **Cada bucle de ventana llama a `VentanaOverlay::seguir_sitio(&mut marco)`
  al empezar la vuelta**, justo despues de `bombear_pendientes()` y antes de
  repartir los eventos. Pone el origen del `marco` donde esta la ventana de
  verdad y conserva su tamano (la superficie se hizo para ese).
- `VentanaOverlay::area()` da el tamano pedido **en el sitio real**; quien
  reparte clics con `area()` (la pastilla de pantalla, la barra del GIF) ya
  lo hace bien.
- Las ventanas con `PinLector` (visores, lector de PDF, lienzo, estudiar
  clase…) siguen la ventana con `seguir_ventana`, que ademas toma el tamano.
- Los pines (`pixpin-pin`) atienden `WM_WINDOWPOSCHANGED`: con el mismo
  tamano y otro sitio que el esperado, el contenido va con la ventana
  (`marco_vivo::contenido_seguido`).
- Lo que hable directamente con Win32 usa el `lparam` del mensaje (cliente,
  con signo: `as i16 as i32`) o `ScreenToClient` en el momento.

La aritmetica pura esta en `pixpin_geom::marco_vivo`, con sus pruebas.
Una ventana nueva que se salte `seguir_sitio` vuelve a tener el fallo.

## Arrastrar por la cabecera

Las ventanas de PinDesk no tienen barra de Windows. Las que se mueven por su
cuenta (la caja de soltar, las flotantes, la galeria, tareas…) lo hacen al
arrastrar su cabecera. Las demas le dicen a Windows por donde se arrastran
con `VentanaOverlay::poner_arrastre(zonas, botones)`, en pixeles del
cliente: dentro de una zona y fuera de todos los botones, `WM_NCHITTEST`
contesta `HTCAPTION` y Windows la mueve como a cualquier otra.

- Ajustes: la franja del titulo encima del buscador y la cabecera de la
  seccion, sin «Restablecer» (`pixpin_ui::ajustes::zonas_de_arrastre`).
- Repaso oral, conversacion y Pronunciar: la franja de arriba sin sus
  botones (`ventanita::arrastre_por_arriba`, tras pintar).

Ninguna zona de arrastre puede tapar algo que se pulse: la prueba
`la_cabecera_vacia_arrastra_y_no_le_quita_el_clic_a_nada` lo comprueba en
los ajustes, y `movida_con_altsnap_la_seccion_se_sigue_pulsando_donde_se_ve`
(Windows, `--ignored`) mueve la ventana con `SetWindowPos` y pulsa.
