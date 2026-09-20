# PixPin Max — English strings.

app-nombre = PixPin Max


resultado-copiar = Copy
resultado-guardar-como = Save as…
resultado-guardar = Save
resultado-descartar = Discard

error-otra-instancia = PixPin Max is already running.
error-atajo-ocupado = Could not register { $atajo }: another application is using it.

captura-guardada = Screenshot saved to { $ruta }
captura-fallo = Could not capture the screen: { $motivo }

barra-copiar = Copy
barra-guardar = Save
barra-guardar-como = Save as…
barra-descartar = Discard

pin-no-encontrado = Not found

pin-copiar = Copy
pin-guardar-como = Save as…
pin-abrir-ubicacion = Open file location
pin-tamano-original = Original size
pin-grupo = Group
pin-sin-grupo = No group
pin-color-rojo = Red
pin-color-naranja = Orange
pin-color-ambar = Amber
pin-color-verde = Green
pin-color-cian = Cyan
pin-color-azul = Blue
pin-color-violeta = Violet
pin-color-rosa = Pink
pin-ocultar-grupo = Hide this group
pin-cerrar = Close
pin-eliminar = Delete from store…
pin-eliminar-confirmar = This will permanently delete it from the store. Are you sure?
grupos-ocultos = Hidden groups

capa-guardar-titulo = Screen annotation
capa-guardar-pregunta = Keep the drawing as a pin?
pin-reproducir = Play
pin-pausar = Pause
pin-sonido = Sound
barra-todo = Select all
pin-sin-codec = no video codec
comando-capturar-region = Capture
comando-capturar-y-copiar = Capture and copy
comando-capturar-con-scroll = Scrolling capture
comando-cuentagotas = Colour picker
comando-pinear = Crop and pin
comando-pinear-en-vivo = Live pin (watch an area live)
comando-pinear-portapapeles = Pin the clipboard
comando-anotar = Annotate the screen
comando-anotar-congelada = Annotate a screenshot
comando-abrir-chat = Open the project chat
comando-abrir-ajustes = Settings
comando-salir = Quit
comando-alternar-pines = Hide or show pins
comando-restaurar-ultimo-pin = Reopen last closed pin
comando-cerrar-todos-los-pines = Close all pins
comando-ventana-encima = Keep the window under the pointer on top
comando-copiar-texto = Copy text from an area
comando-capturar-y-anotar = Capture and annotate
comando-capturar-ultima-region = Repeat last capture
comando-grabar-gif = Record an area as GIF

# The GIF recording bar (P5b).
grabar-empezar = Record
grabar-parar = Stop
grabar-pausa = Pause
grabar-seguir = Resume
grabar-cerrar = Close
grabar-por-segundo = { $ritmo }/s
grabar-cuenta-atras = { $segundos }...

# The editor that opens when recording stops (P5b.4).
editor-ver = Play
editor-pausar = Pause
editor-anterior = Previous
editor-siguiente = Next
editor-velocidad = { $veces }x
editor-guardar = Save
editor-guardado-rapido = Quick
editor-copiar = Copy
editor-descartar = Discard

# Click-through pins (P1.4).
comando-alternar-paso-de-clics = Let clicks pass through pins
pin-dejar-pasar-clic = Let clicks pass through

# Muting the shortcuts for a while (P1.7).
comando-silenciar-atajos = Mute the shortcuts
bandeja-silenciada = PixPin Max — shortcuts muted
aviso-atajos-silenciados = Shortcuts muted
aviso-atajos-silenciados-detalle = Turn them back on from this same menu.
aviso-atajos-activos = Shortcuts active
aviso-atajos-activos-detalle = { $cuantos } shortcuts respond again.

# Delayed capture (P2.1).
comando-capturar-con-retardo = Capture after a countdown
cuenta-atras-cancelar = Esc to stop

# Pinning the Explorer selection (P1.6).
comando-pinear-seleccion = Pin the Explorer selection

# The settings window (P6).
ajustes-pestana-atajos = Shortcuts
ajustes-pestana-general = General
ajustes-pestana-captura = Capture
ajustes-pestana-dibujo = Drawing
ajustes-iman = Snap to notable points
ajustes-iman-esquinas = Corners and endpoints
ajustes-iman-medios = Midpoints
ajustes-iman-centros = Centres
ajustes-iman-radio = Grab distance (px)
ajustes-idioma = Language
ajustes-idioma-sistema = Same as Windows
ajustes-arranque = Start with Windows
ajustes-color = Color picker format
ajustes-nivel = Performance
ajustes-nivel-auto = Automatic
ajustes-nivel-completo = Full
ajustes-nivel-ligero = Light
ajustes-retardo-captura = Countdown seconds before capturing
ajustes-limite-scroll = Maximum height of a scrolling capture, in pixels
ajustes-gif-ritmo = Frames per second when recording
ajustes-gif-retardo = Grace seconds before recording
ajustes-sin-atajo = No shortcut
ajustes-pulsa-combinacion = Press the combination…
ajustes-choca = duplicate
ajustes-cerrar = Close
ajustes-abrir-fichero = Open the settings file

# Reading the text of a pin (P4.2).
pin-copiar-texto = Copy the text in the image

# Opening an image pin in the editor canvas.
pin-abrir-en-lienzo = Open in canvas
pin-congelar = Freeze as image

# Pages of a pinned PDF.
pin-pagina-siguiente = Next page
pin-pagina-anterior = Previous page
pin-extraer-pagina = Extract this page as a pin
pin-extraer-todas = Extract every page
aviso-paginas-extraidas = Pages extracted
aviso-paginas-extraidas-detalle = { $hechas } of { $total }. The rest were left out so the screen stays usable.
chat-titulo = Projects
chat-sin-proyectos = No projects yet
chat-elige-proyecto = Pick a project to see it
chat-hojas = { $cuantas ->
    [one] 1 sheet
   *[other] { $cuantas } sheets
  }
chat-sin-mensajes = Nothing in this project yet
chat-clase-imagen = Image
chat-clase-archivo = File
chat-clase-voz = Voice note
chat-clase-dibujo = Drawing
chat-clase-pagina = Page
chat-clase-proyecto = Project
chat-clase-miniapp = Mini app
chat-escribe = Write something…
# The chat, like PixPin Android's (guardados/MensajesActivity.kt).
chat-cosas = { $n ->
    [one] 1 thing
   *[other] { $n } things
}
chat-elegidos = { $n ->
    [one] 1 selected
   *[other] { $n } selected
}
chat-lienzo = Canvas
chat-tabla = Table
chat-viene-de = Comes from
chat-comentar = Comment on this
chat-copiar = Copy
chat-fijar = Pin to top
chat-soltar = Unpin from top
chat-compartir = Share
chat-abrir-con = Open with another app
chat-aligerar = Make the PDF lighter
chat-renombrar = Rename
chat-pinear = Put on screen
chat-recordar = Remind me
chat-rescatar = Keep this
chat-solo-la-foto = Show only the photo
chat-ver-hilo = See comments
chat-etiquetar = Add a tag
chat-quitar-etiqueta = Remove the tag
chat-reenviar = Forward
chat-unir = Add to project
chat-transcribir = Convert to text
chat-letra = Lyrics or text
chat-elegir = Select
chat-borrar = Delete
chat-borrar-aviso = It will be removed from the conversation. An attached file is deleted with it. Continue?
chat-biblioteca-audio = Audio library
chat-conversaciones = Project conversations
chat-proyectos = Projects
chat-comenzar = Start
chat-fondo = Chat background
chat-ajustes = Settings
chat-fondo-mar = Sea
chat-fondo-arena = Sand
chat-fondo-brezo = Heather
chat-fondo-bosque = Forest
chat-fondo-ocaso = Sunset
chat-fondo-pizarra = Slate
chat-adj-archivo = A file
chat-adj-pagina = A page from a project
chat-adj-proyecto = A whole project
chat-adj-conversacion = A conversation
chat-adj-telepronter = Teleprompter
chat-adj-pronunciar = Pronunciation
chat-adj-miniapp = A mini-app
chat-reenviado = Forwarded
chat-copiado = Copied
chat-compartido = Copied to the clipboard: paste it wherever you want to share it
chat-pineado = Put on screen
chat-sin-archivo = This message has no file on this computer
chat-sin-origen = That canvas is not in this project
chat-sin-proyectos-otros = There is no other project to send it to
chat-ya-en-marcha = PixPin is already running on Windows: pins come out with its shortcuts
chat-ajustes-bandeja = Settings open from the PixPin icon in the tray
chat-proyectos-lista = Projects are the list on the left
chat-no-hay-voz = Recording voice notes does not exist on Windows yet
chat-no-hay-recordatorios = Reminders do not exist on Windows yet
chat-no-hay-transcripcion = Converting audio to text does not exist on Windows yet
chat-no-hay-letra = Audio lyrics do not exist on Windows yet
chat-no-hay-aligerar = Making a PDF lighter does not exist on Windows yet
chat-no-hay-unir = On Windows every chat is already a project: use Forward to take it to another
chat-no-hay-biblioteca = The audio library does not exist on Windows yet
chat-no-hay-recorte = Show only the photo does not exist on Windows yet
chat-no-hay-pagina = Attaching a page from a project does not exist on Windows yet
chat-no-hay-conversacion = Recording a conversation does not exist on Windows yet
chat-no-hay-telepronter = The teleprompter does not exist on Windows yet
chat-no-hay-pronunciar = Pronunciation practice does not exist on Windows yet
chat-no-se-pudo = It could not be done: check the PixPin log
chat-fecha = { $mes } { $dia }
chat-fecha-con-ano = { $mes } { $dia }, { $ano }
chat-mes-1 = January
chat-mes-2 = February
chat-mes-3 = March
chat-mes-4 = April
chat-mes-5 = May
chat-mes-6 = June
chat-mes-7 = July
chat-mes-8 = August
chat-mes-9 = September
chat-mes-10 = October
chat-mes-11 = November
chat-mes-12 = December
chat-hoy = Today
chat-ayer = Yesterday
chat-buscar = Search
chat-adjuntar = Attach
chat-fijado = Pinned message
info-todo = All
info-fotos = Photos
info-archivos = Files
info-voz = Voice
info-dibujos = Drawings
info-fijados = Pinned
info-buzon = Inbox
info-titulo = Info
adjuntar-imagen = Photo or video
adjuntar-archivo = File
adjuntar-lienzo = New canvas
confirmar-titulo = { $cuantos ->
    [one] Add one file
   *[other] Add { $cuantos } files
}
confirmar-resto = and { $cuantos } more
confirmar-pie = Add a comment…
confirmar-cancelar = Cancel
confirmar-aceptar = Add
adjuntar-tabla = New table
tabla-nueva = Table
chat-nombre-nuevo = Project name…
hoja-volver = Esc to go back
menu-foto-lienzo = Open in canvas
menu-foto-pin = Pin it again
menu-foto-abrir = Open with Windows
recibir-titulo = Receive from phone
recibir-como = Scan the code with PixPin on your phone
recibir-esperando = Waiting for the phone…
recibir-hecho = { $cuantos } items arrived
recibir-salir = Escape to close
comando-recibir-del-movil = Receive from phone…
comando-sincronizar = Sync
adjuntar-del-movil = Bring from phone…
menu-foto-aqui = Draw right here
proyecto-borrar = Delete project
proyecto-borrar-varios = Delete selected
proyecto-borrar-aviso = It will leave the list and its folder will go to the PixPin bin. Continue?

## Sync (copy of the phone's SincronizarActivity.kt)
sinc-titulo = Sync
sinc-subtitulo = Over Wi-Fi, no internet.
sinc-este-aparato = This device
sinc-tus-aparatos = Your devices
sinc-sin-grupo = Create a group on one of your devices and join from the others with its code. Only devices in the group sync with each other.
sinc-crear-grupo = Create a group
sinc-unirme = Join with a code
sinc-grupo-vacio = There is no other device in the group yet. Add one with the code below.
sinc-disponible = Available
sinc-no-encontrado = Not found
sinc-sincronizar = Sync
sinc-probar = Try
sinc-con-todos = Sync with all ({ $n })
sinc-con-todos-como = Without asking: uses what you chose last time with each one.
sinc-si-no-aparece = If it does not show up: it needs PixPin open and this Wi-Fi.
sinc-con-direccion = Connect to an address
sinc-anadir = Add another device
sinc-anadir-como = On the other device: Sync → “Join with a code”.
sinc-toca-codigo = Click to see the code
sinc-salir = Leave the group
sinc-pasar = Send something to someone
sinc-pasar-como = Just once, to anyone with PixPin on your Wi-Fi. No group needed.
sinc-recibir = Receive
sinc-enviar = Send
sinc-pasar-tambien = What arrives goes to the project list or to “Saved messages”.
sinc-actividad = Activity
sinc-escuchando = This computer can be found at { $dir } while this window is open.
sinc-nombre-titulo = Name of this device
sinc-nombre-como = This is how your other devices will see it.
sinc-unirme-titulo = Join a group
sinc-unirme-como = Type the code shown under “Add another device” on a device in the group. That device needs PixPin open and your same Wi-Fi.
sinc-codigo = Code
sinc-direccion = Address (only if it is not found)
sinc-direccion-titulo = Connect to an address
sinc-direccion-como = Type the address shown on the other device, in this same window.
sinc-la-de-este = This device's:
sinc-salir-titulo = Leave the group?
sinc-salir-como = This device will stop syncing with the others. Nothing is deleted. If you join again, the first time it will ask about what changed on both sides.
sinc-salir-boton = Leave
sinc-un-momento = One moment
sinc-no-se-pudo = It didn't work
sinc-guardar = Save
sinc-unirme-boton = Join
sinc-conectar = Connect
sinc-cancelar = Cancel
sinc-ocultar = Hide
sinc-vale = OK
sinc-con-todos-rondas = Without asking, one after another and in two rounds, so that everyone ends up with everything.
sinc-elegir-titulo = What to sync?
sinc-elegir-con = With { $otro }. What you tick ends up the same on both.
sinc-en-los-dos = On both
sinc-solo-en-uno = Only on one
sinc-este-aparato-n = This device · { $n }
sinc-elegir-como = What changed on both is merged: shape by shape, cell by cell, paragraph by paragraph. If both touched the same thing, the last change wins, and the earlier one stays in «Backups».
sinc-todo = All
sinc-nada = None
sinc-sincronizar-n = Sync { $n }
sinc-sincronizando = Syncing

## The universe
universo-titulo = Universe
universo-cosmos = Cosmos
universo-buscar = Search…
universo-archivos = { $n ->
    [one] { $n } file
   *[other] { $n } files
  }
universo-recuento = { $p ->
    [one] { $p } project
   *[other] { $p } projects
  } · { $a ->
    [one] { $a } file
   *[other] { $a } files
  } · { $c ->
    [one] { $c } connection
   *[other] { $c } connections
  }
universo-nebulosa = Nebula
universo-mas = +{ $n } more
universo-nuevas = { $n } new
universo-abrir = Open
universo-ir-chat = Go to chat
universo-carpeta = Show in folder
universo-devolver = Return to the nebula
universo-nota = Note
universo-conexiones = Connections
universo-notas-chat = Include chat notes
universo-ordenar = Arrange planets
universo-limpiar = Clean up orphans
universo-agrupar = Group into new planet
universo-conectar = Connect together
universo-no-esta = This file is not on this computer
universo-roto = The saved universe could not be read; a copy was set aside
universo-no-guardado = The universe could not be saved. Close anyway?
universo-borrar-anotaciones = { $n } annotations will be deleted. Continue?
universo-abrir-varios = { $n } of { $total } files will be opened. Continue?
universo-rotas = { $n } unreadable messages
universo-tipo-relacion = Related
universo-tipo-depende = Depends on
universo-tipo-referencia = Reference
universo-tipo-secuencia = Sequence
universo-planeta = Planet
universo-exoplaneta = Exoplanet
universo-galaxia = Galaxy
universo-luna = File
universo-varios = { $n } items
universo-abrir-todo = Universe
universo-ver-proyecto = Show in the universe
menu-foto-universo = Show in the universe
bandeja-universo = Universe

## Receive over Wi-Fi (copy of the phone's RecibirActivity.kt)
rw-titulo = Receive over Wi-Fi
rw-misma-wifi = You have to be on the same Wi-Fi as the sender (or connected to their hotspot).
rw-o-escribe = or type the code
rw-pista-pega = The computer has no camera: type the six digits or paste the text of their QR with Ctrl+V.
rw-recibir = Receive
rw-prefieres = Rather be scanned? Show your own code and let the sender point their camera at it.
rw-ensenar = Show my code so they can send to me
rw-que-te-escaneen = Let them scan this
rw-esperando-mio = Waiting for someone to send you something. You can also read this code out for «Send → Scan the receiver».
rw-buscando = Looking for the sender…
rw-conectando = Connecting…
rw-no-encontrado = Nobody is sending with that code. Check the code and that you are both on the same Wi-Fi (or on one's hotspot).
rw-codigo-malo = That code is not the right one. Check it.
rw-se-corto = Cut off: { $motivo }
rw-quiere = { $de } wants to send you
rw-aparato = Device { $codigo }
rw-proyecto = Project «{ $nombre }»
rw-lienzo = Canvas «{ $nombre }»
rw-nada-coincide = None of this matches what you have: it comes in as new, without touching anything of yours.
rw-no-gracias = No, thanks
rw-aceptar = Accept
rw-recibiendo = Receiving from { $de }
rw-de = { $hechos } of { $total }
rw-recibido = Received from { $de }
rw-no-guardado = It arrived, but could not be saved.
rw-en-proyectos = { $nombre }: in the project list
rw-en-guardados = { $nombre }: in «Saved messages»
rw-en-carpeta = { $nombre }: only in the «recibidos» folder
rw-no-se-pudo = It didn't work
rw-probar-otra-vez = Try again
rw-cerrar = Close

## Send over Wi-Fi (copy of the phone's EnviarActivity.kt)
ew-titulo = Send over Wi-Fi
ew-que = What do you want to send?
ew-que-como = On the phone you choose when sharing; here, files from this computer or a whole project from the chat.
ew-archivos = Files
ew-un-proyecto = A project
ew-elige-proyecto = Choose the project
ew-sin-proyectos = There are no projects yet.
ew-preparando = Preparing…
ew-no-preparado = What you wanted to send could not be prepared.
ew-vas-a-enviar = You are sending
ew-vas-a-enviar-n = You are sending { $n } things
ew-y-mas = and { $n } more
ew-un-aparato = One device
ew-n-aparatos = { $n } devices
ew-pidiendo = Wants to receive what you send
ew-enviando = Sending · { $hechos } of { $total }
ew-esperando-acepte = Waiting for them to accept…
ew-enviado = Sent
ew-no-acepto = Declined
ew-no-aprobado = Not approved
ew-no = No
ew-enviar = Send
ew-enviar-a-todos = Send to everyone waiting
ew-pegar = Paste the receiver's code
ew-pegar-como = On the other device: Receive over Wi-Fi → «Show my code so they can send to me». Type its six digits here or paste the text of its QR with Ctrl+V.
ew-mandar = Send it
ew-dejar-de-pegar = Stop pasting
ew-en-el-otro = On the other phone: PixPin → Sync → Receive
ew-o-escribe = or type this code
ew-misma-wifi = Both on the same Wi-Fi
ew-misma-wifi-como = If there is no Wi-Fi, one can turn on their hotspot and the other join it. No internet is used.
ew-malos = Someone tried a wrong code ({ $n } of { $de }).
ew-esperando = Waiting… Several devices can connect; you press the button for each one. The code stops working when this screen closes.
ew-quemado = Someone tried a wrong code several times, so this code no longer works. Send again to get another one.
ew-cerrar = Close

## «Update mine / Create as new» when receiving (RecibirActivity.kt)
rw-ya-tienes = You already have «{ $nombre }» with the same codes
rw-actualizar = Update the one I have
rw-como-nuevo = Create as new
rw-encima-de-lo-tuyo = It is written over yours. What was there stays in «Backups».
rw-entra-aparte = It comes in separately, with new codes: from now on it is another thing.
rw-proyecto-al-dia = «{ $nombre }» brought up to date
rw-guardados-al-dia = «{ $nombre }» brought up to date in Saved messages

# The header magnifier, which searches inside the conversation (task 1.3).
chat-buscar-aqui = Search here…
chat-buscar-nada = Nothing matches
chat-buscar-resultados = { $n ->
    [one] 1 result
   *[other] { $n } results
}
chat-buscar-en-contexto = See in context
chat-pagina-n = Page { $n }
chat-unido-a = Added to "{ $nombre }"
chat-unir-nada = Nothing here can be added: only photos, drawings, notes and PDFs
chat-devolver = Add back to the project
chat-devuelto = Back in "{ $nombre }"
chat-devolver-no = Could not add it back

## Backups (copy of sincro/CopiasActivity.kt from the phone)
cop-titulo = Backups
cop-portada-como = How each conversation was before receiving something over Wi-Fi and before each sync.
cop-ver = See the backups
cop-versiones = Earlier versions
cop-como-estaba = How the project was before each received transfer and each sync. Going back to one does not delete what you did afterwards.
cop-todavia-nada = No backups yet. They are made on their own before receiving something over Wi-Fi and before syncing.
cop-como-se-hacen = The last 30 of each conversation are kept. Files over 40 MB are not in the backup: there is no going back for those.
cop-hojas-mensajes = { $hojas } sheets · { $mensajes } messages
cop-sin-grandes =  · without the PDF (too large)
cop-volver = Go back
cop-cancelar = Cancel
cop-volver-titulo = Go back to this backup?
cop-volver-texto = «{ $nombre }» goes back to how it was on { $cuando } ({ $motivo }): { $hojas } sheets. The canvases you added afterwards stay. A backup of how it is now is saved first, so you can undo this.
cop-ha-vuelto = «{ $nombre }» has gone back to the backup
cop-no-se-pudo = Could not go back to the backup: { $motivo }

## The document viewer: Word, EPUB books and web pages (task 2.1)
visor-titulo = PixPin viewer
visor-marcador = Bookmark here
visor-ajustes = Reading settings
visor-tamano = Text size
visor-quitar-marcadores = Remove bookmarks
visor-guardar-pagina = Save as a web page
visor-guardar-pdf = Save as PDF
visor-abrir-carpeta = Open the folder
visor-guardado = Saved to "{ $nombre }"
visor-no-guardado = Could not save it
visor-vacio = This document has no text to show
visor-ayuda = Wheel to read · +/− size · M bookmark · G settings · Esc to leave
visor-elige-emoticono = What do you mark it with?
visor-sin-marcadores = No bookmarks yet
sinc-presencia-apagada = Presence is off in the settings ([sincro] presencia = false): you can sync from here, but the phone will not find this computer or be able to call it.
bandeja-abrir-documento = Open a document…

## Canvas shapes and ink: arrowheads, fills and text style (group A)
lienzo-punta-ninguna = No arrowhead
lienzo-punta-flecha = Arrow
lienzo-punta-barra = Bar
lienzo-punta-circulo = Circle
lienzo-punta-circulo-hueco = Circle outline
lienzo-punta-triangulo = Triangle
lienzo-punta-triangulo-hueco = Triangle outline
lienzo-punta-rombo = Diamond
lienzo-punta-rombo-hueco = Diamond outline
lienzo-punta-inicio = Start arrowhead
lienzo-punta-fin = End arrowhead
lienzo-trama-zigzag = Zigzag
lienzo-trama-tiralineas = Ruled lines
lienzo-texto-negrita = Bold
lienzo-texto-cursiva = Italic
lienzo-texto-tachado = Strikethrough
lienzo-presion-firme = Fixed-width stroke
lienzo-flecha-codos = Elbow arrow
lienzo-flecha-libre = Freehand arrow
lienzo-arco-guia = Guide oval, not traced yet
lienzo-luces-fuerza = Light strength

## Build and measure: fill, trim, extend, points and angles (group C)
lienzo-bote = Fill bucket
lienzo-bote-ayuda = Click inside a closed gap and it gets filled, even if several different shapes form it.
lienzo-bote-abierto = This gap is not closed: the paint would leak out through a crack.
lienzo-recortar = Trim
lienzo-recortar-ayuda = Click the leftover piece of a stroke and it goes, up to where the others cross it.
lienzo-extender = Extend
lienzo-extender-ayuda = Click the end that falls short and it reaches the first thing in its way.
lienzo-extender-sin-tope = Nothing in its way: a line stretched into the void is not an extended line.
lienzo-punto = Labelled point
lienzo-punto-ayuda = Click a crossing, an endpoint or the centre of a circle and the point appears with its letter.
lienzo-punto-sin-sitio = There is nothing to name there: a point goes on a crossing, an endpoint or a centre.
lienzo-punto-serie-mayusculas = Capital letters (A, B, C…)
lienzo-punto-serie-minusculas = Small letters (a, b, c…)
lienzo-punto-serie-numeros = Numbers (1, 2, 3…)
lienzo-angulos = Live angles
lienzo-angulos-ayuda = While you move something, it shows how big the corners you are touching are.
lienzo-area = Area

## Select, transform and bind (group B)
lienzo-lazo = Lasso
lienzo-lazo-ayuda = Draw around whatever you want to pick. What falls fully inside is taken; with Shift, whatever the stroke touches too.
lienzo-lazo-encerrar = Only what falls inside
lienzo-lazo-rozar = Whatever the stroke touches too
lienzo-voltear-horizontal = Flip horizontally
lienzo-voltear-vertical = Flip vertically
lienzo-voltear-ayuda = Mirrors the selection about the centre of the whole group, not each shape about itself.
lienzo-rejilla = Ruled paper
lienzo-rejilla-ninguna = Plain
lienzo-rejilla-lineas = Squares
lienzo-rejilla-puntos = Dots
lienzo-rejilla-imanta = Snap the cursor to the squares
lienzo-rejilla-ayuda = The reference that is there before the first stroke. It does not show up when exporting.
lienzo-iman-intersecciones = Crossings between shapes
lienzo-iman-intersecciones-ayuda = The point where two strokes cut each other, which exists as a corner of neither.
lienzo-iman-bordes = Shape edges
lienzo-iman-bordes-ayuda = The set square: the cursor slides along the whole edge, not just its corners.
lienzo-iman-eje = Origin and coordinate axes
lienzo-enganche = Bind the arrow to the shape
lienzo-enganche-ayuda = Drop the tip on a shape and the arrow will follow it when you move it.
lienzo-enganche-soltar = Release the tip
lienzo-tirador-punta = Move the end
lienzo-tirador-anadir = Bend here
lienzo-copiar-estilo = Copy style
lienzo-copiar-estilo-ayuda = Take the colour, stroke and fill from one shape and paste them on others. Only what both types accept travels.
lienzo-estilo-tomado = Style taken
lienzo-estilo-sin-tomar = You have not taken a style yet.

# The attachment the document viewer can open without leaving PixPin.
chat-abrir-aqui = Open here
