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
sinc-pasar-tambien = Also from each project's menu in the chat.
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

## The universe
universo-titulo = Universe
universo-cosmos = Cosmos
universo-buscar = Search…
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
