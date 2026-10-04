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
ajustes-pdf-aligerar = Make PDFs lighter when added to the chat
ajustes-pdf-nivel = How much lighter (Small and Max: by hand only)
ajustes-pdf-nivel-sin-perdida = Exact
ajustes-pdf-nivel-equilibrado = Medium
ajustes-pdf-nivel-pequeno = Small
ajustes-pdf-nivel-extremo = Max
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
pin-manejar = Control remotely
pin-dejar-de-manejar = Stop controlling remotely

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
chat-compartir = Share…
chat-abrir-con = Open with another app
chat-aligerar = Make the PDF lighter
chat-renombrar = Rename
chat-renombrar-barra = Name:
chat-renombrar-teclas = Enter saves · Esc keeps the old one
chat-describir = Add caption
chat-describir-editar = Edit caption
chat-describir-barra = Caption:
chat-describir-teclas = Enter saves · Esc keeps it as it was
chat-pinear = Pin
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
chat-escala = Interface scale
chat-escala-normal = monitor default
chat-escala-puesta = Interface scale: { $cuanto } %
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
chat-compartir-sin-panel = Windows did not open the Share panel: it is on the clipboard so you can paste it wherever you want
chat-pineado = Pinned
chat-sin-archivo = This message has no file on this computer
chat-sin-origen = That canvas is not in this project
chat-sin-proyectos-otros = There is no other project to send it to
chat-ya-en-marcha = PixPin is already running on Windows: pins come out with its shortcuts
chat-ajustes-bandeja = Settings open from the PixPin icon in the tray
chat-proyectos-lista = Projects are the list on the left
chat-no-hay-voz = Recording voice notes does not exist on Windows yet
chat-no-hay-aligerar = Making a PDF lighter does not exist on Windows yet
chat-no-hay-unir = On Windows every chat is already a project: use Forward to take it to another
chat-no-hay-biblioteca = The audio library does not exist on Windows yet
chat-ver-dibujo-entero = Show the whole drawing
chat-no-hay-pagina = Attaching a page from a project does not exist on Windows yet
chat-no-hay-telepronter = The teleprompter does not exist on Windows yet
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
info-galeria = Gallery
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
confirmar-pie = Add a caption…
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
proyecto-renombrar = Rename
proyecto-borrar = Delete project
proyecto-borrar-varios = Delete selected
proyecto-borrar-aviso = It will leave the list and its folder will go to the PixPin bin. Continue?
proyecto-papelera = Deleted projects…
# Where each project is saved: its chosen folder or the usual place.
ubicacion-cambiar = Change location…
ubicacion-habitual = Move back to the usual location
ubicacion-abrir = Open project folder
ubicacion-elegir-titulo = Choose where to save the project
ubicacion-nuevo = New project
ubicacion-nuevo-en = New project in a folder…
ubicacion-no-disponible = Location not available
ubicacion-no-disponible-abrir = This project's folder is not available ({ $ruta }). Connect the drive and try again.
ubicacion-movido = Project saved in { $ruta }
ubicacion-vuelto = The project is back in the usual location
ubicacion-error-red = Projects can only be saved on a drive of this computer, not on a network drive.
ubicacion-error-no-es-carpeta = That folder does not exist on this computer.
ubicacion-error-dentro = That folder is inside the project itself or inside PixPin's data.
ubicacion-error-mover = The project could not be moved; it is still where it was.
ubicacion-error-abrir = The project folder could not be opened.

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
sincro-borrado-alli-vuelve = Deleted on { $otro } · will be sent back
sincro-borrado-alli-se-borra = Deleted on { $otro } · WILL BE DELETED HERE (a backup is kept)
sincro-borrado-alli-se-queda = Deleted on { $otro } · unchecked: stays here
sincro-se-borran-aqui = Will be deleted HERE, because it was deleted on { $otro }: { $lista }. Uncheck it to keep it, or choose «Mine wins» to send it back.
sincro-se-borran-alli = Will be deleted on { $otro }, because you deleted it here: { $lista }.
sincro-juntar = Merge
sincro-lo-mio-manda = Mine wins
sincro-lo-mio-manda-como = One way only: what { $yo } has overwrites { $otro }. Nothing is deleted or changed here; there, whatever differs ends up as here and what they deleted comes back. Whatever only { $otro } has is kept. A backup of what gets overwritten is kept.
sincro-mandar-n = Send { $n }

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
unir-pdf-como-imagenes = Add as images
unir-pdf-empezado = Adding the PDF to the project…
unir-pdf-ya-va = That PDF is already being added
unir-pdf-no = Could not add the PDF
unir-pdf-hecho = { $hojas ->
    [one] Added 1 sheet to the project
   *[other] Added { $hojas } sheets to the project
}
unir-pdf-pintadas = { $hojas ->
    [one] Added 1 sheet (the PDF could not be copied: it was drawn)
   *[other] Added { $hojas } sheets (the PDF could not be copied: they were drawn)
}
unir-pdf-fotos = { $hojas ->
    [one] Added 1 page as an image
   *[other] Added { $hojas } of { $paginas } pages as images
}
unir-pdf-pintando = Drawing pages… { $hechas } of { $total }
unir-pdf-imagenes-progreso = Turning into images… { $hechas } of { $total }

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
papelera-titulo = Deleted projects
papelera-como = They come back whole: with their name, their sheets in order, their PDF and their chat. And they stop being deleted, so syncing won't remove them again.
papelera-detalle = { $hojas } sheets · { $mensajes } messages · deleted on { $cuando }
papelera-recuperar = Recover
papelera-recuperar-todos = Recover all
papelera-ha-vuelto = «{ $nombre }» is back, whole
papelera-no-se-pudo = Could not recover «{ $nombre }»: { $motivo }
papelera-recuperados = Recovered { $n } of { $total } projects

## The document viewer: Word, EPUB books and web pages (task 2.1)
visor-html-titulo = PixPin — page
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

## The readers: comfortable reading, annotating on top and the PDF reader
lector-listo = Done
lector-indice = Contents
lector-letra-fijada = The type is locked: there are notes drawn on top
lector-letra-normal = Normal type
lector-letra-fija = Monospace
lector-grosor-normal = Regular
lector-grosor-gruesa = Bold
lector-quitar-tinta = Remove the notes
lector-tinta-quitada = Notes removed (Ctrl+Z brings them back while you stay here)
lector-ayuda-docs = Wheel to read · Ctrl+wheel zoom · L listen · A annotate · M bookmark · I contents · +/− size · T type · B weight · Esc to leave
lector-pdf-titulo = PixPin PDF reader
lector-pdf-no-se-abre = Could not open the PDF
lector-pdf-abriendo = Opening the PDF…
lector-exportando = Saving the annotated PDF… { $hechas } of { $total }
lector-exportar-pdf = Save the PDF with the notes
lector-anotado = annotated
lector-ayuda-pdf = Wheel to turn pages · Ctrl+wheel zoom · Ctrl+[ Ctrl+] space to annotate · L listen · A annotate · M bookmark · Esc to leave
lector-escuchar = Listen (L)
lector-sin-texto = This document has no text to read
lector-leyendo-texto = Getting the text out of the PDF to read it…
lector-sin-voz = Windows has no voice for “{ $idioma }”: add one in Settings > Time & language > Speech
lector-voz-idioma = en-US
lector-verde-aqui = Green bookmark here: reading will start from this point
lector-espacio-tope = No more room on that side
lector-espacio-nada = That side has no space left
lector-sin-lado = Sideways locked: it only scrolls up and down
lector-con-lado = It moves everywhere
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

# The seven mini-apps from the phone, and their panel.
mini-moneda = USD
mini-tareas = To-do list
mini-gastos = Expenses
mini-cronometro = Stopwatch
mini-temporizador = Timer
mini-contador = Counter
mini-ruleta = Spinner
mini-alarma = Alarm
mini-tarea-nueva = Something to do, then Enter · ↑↓ pick, Space ticks, F2 edits
mini-gasto-nuevo = Item and amount (Dinner 42.50), then Enter · ↑↓ pick, F2 edits
mini-nombre-nuevo = A name, then Enter · Enter on empty spins, Del removes
mini-limpiar-hechas = Clear the done ones
mini-ocultar-hechas = Hide the done ones
mini-mostrar-hechas = Show the done ones
mini-avance = { $hechas } of { $de } done
mini-tarea-edad = { $dias ->
    [0] today
    [one] 1 day ago
   *[other] { $dias } days ago
  }
mini-arrancar = Start
mini-parar = Stop
mini-vuelta = Lap
mini-reiniciar = Reset
mini-sortear = Spin
mini-alarma-encender = Turn on
mini-alarma-apagar = Turn off
mini-guia-titulo = New name, then press Enter (Esc keeps the old one)
mini-guia-cambiar = Fix it and press Enter (Esc leaves it as it was)
mini-guia-duracion = Minutes or m:ss, then Enter · Space starts
mini-guia-hora = Time (7:30), then Enter · Space turns it on
chat-biblioteca-vacia = No voice notes or songs yet.
chat-biblioteca-musica = Music
chat-biblioteca-notas = Voice notes

# Recording a voice note from the chat.
chat-voz-muy-corta = Too short: hold it a little longer.
chat-voz-sin-microfono = No microphone, or PixPin isn't allowed to use it. Check Settings › Privacy › Microphone.
chat-voz-sin-codec = This Windows has no audio encoder (usually an N edition without the media feature pack).
chat-voz-fallo = The voice note couldn't be recorded.

# Shrinking an attached PDF: it lowers the photos inside, and never for the worse.
pdf-aligerado = Shrunk: from { $antes } to { $despues }
pdf-aligerar-cifrado = The PDF is encrypted and can't be rewritten without risking it.
pdf-aligerar-indice-comprimido = This PDF keeps its index compressed, and rebuilding that isn't supported here yet.
pdf-aligerar-objetos-comprimidos = This PDF keeps its objects compressed, and rebuilding those isn't supported here yet.
pdf-aligerar-sin-trailer = The PDF has no readable trailer.
pdf-aligerar-sin-objetos = No object inside the PDF was recognised.
pdf-aligerar-largo-indirecto = A stream in the PDF doesn't state its length.
pdf-aligerar-objetos-sin-entender = There are parts of the PDF this reader does not understand; rewriting it could drop them.
pdf-aligerar-sin-fotos = It has no photos to lower: the weight is text or vectors.
pdf-aligerar-ya-al-minimo = The photos are already at the minimum.
pdf-aligerar-no-compensa = It would only drop from { $antes } to { $despues }: not worth it.
pdf-aligerar-no-se-lee = The shrunk file didn't read back properly; the original stays.
aligerar-progreso = Making lighter… { $por }%
aligerar-empezado = Making the PDF lighter…
aligerar-sin-mejora = This PDF was already light: it stays as it was.

# Reminders: the time set on a message and the notice that shows up when it
# arrives. The same times the phone offers, plus ten minutes.
chat-recordar-10-min = In 10 minutes
chat-recordar-1-hora = In 1 hour
chat-recordar-3-horas = In 3 hours
chat-recordar-esta-tarde = This evening (18:00)
chat-recordar-manana = Tomorrow (9:00)
chat-recordatorio-puesto = I will remind you at { $cuando }
chat-recordatorio-quitar = Remove the reminder
chat-recordatorio-quitado = Reminder removed
chat-recordatorio-titulo = Reminder

# A message menu entry: send the attachment to someone over Wi-Fi.
chat-enviar-wifi = Send over Wi-Fi

# Turning a voice note into text with Vosk. What is missing is named with its
# file and link: PixPin never downloads anything on its own.
chat-transcribiendo = Converting to text… { $pct }%
chat-transcribir-hecha = Voice note converted to text
chat-transcribir-en-marcha = Another note is already being converted; wait for it to finish
chat-transcribir-sin-mensaje = The note is no longer in the conversation: the text has nowhere to go
chat-transcribir-cancelada = Text conversion was cancelled
chat-transcribir-otra-vez = Convert to text again
chat-transcripcion-no = Could not convert to text: nothing in the audio was understood.
chat-letra-vacia = This note has no text yet.
chat-letra-copiada = Text copied, with its timestamps
chat-recordar-hora = Pick the time…
chat-recordar-hora-a-las = Remind me at
chat-recordar-hora-teclas = Type the time · Enter sets it · Esc leaves it
chat-recordar-hora-queda = Enter: { $cuando }
chat-recordar-hora-mal = That is not a time: type it like 18:30
chat-voz-sin-motor = The speech recogniser is missing: copy { $motor } into { $donde }
chat-voz-sin-modelo = The { $modelo } speech model is missing: download it from { $enlace } and unzip it inside { $donde }
chat-voz-idioma-sin-modelo = Vosk has no speech model for this language
chat-voz-ruta-imposible = The model folder ({ $donde }) has characters the recogniser cannot open; move it to a path without accents
chat-voz-modelo-ilegible = The model in { $donde } is incomplete or could not be loaded
chat-voz-no-es-audio = This note's audio could not be read
chat-voz-audio-vacio = The audio is empty: there is nothing to convert
chat-voz-no-se-entiende = Nothing said in the note could be understood
chat-voz-sin-reconocedor = Windows has no speech recognizer for this language: add it in Settings › Time & language › Speech.
chat-voz-bajar-whisper = Converting to text needs the Whisper speech model (about { $megas } MB). It is downloading now, only this once, and then the note will be converted.
chat-voz-bajando-whisper = Downloading the speech model… { $pct }%
chat-voz-sin-modelo-whisper = The Whisper speech model (about { $megas } MB) is missing: press «Convert to text» again to download it.
chat-voz-sin-onnx = This Windows lacks the AI runtime (onnxruntime.dll): copy it into { $donde } or next to PixPin.
chat-voz-descarga = The speech model could not be downloaded ({ $que }). Check the connection and press «Convert to text» again.
chat-voz-onnx = The Whisper speech engine failed: { $detalle }
chat-dictado-oyendo = Listening… speak, then click the microphone when done
chat-dictado-fallo = Dictation failed: { $razon }
chat-dictado-preparando = Getting the microphone ready…

# The teleprompter: the text scrolls by and you read it aloud while it
# records. There is no synthetic voice anywhere, same as on the phone.
telepronter-titulo = Read aloud
telepronter-ensayar = Rehearse
telepronter-parar = Stop
telepronter-grabar = Record
telepronter-terminar = Finish
telepronter-tirar = Discard
telepronter-velocidad = speed and size
telepronter-preparate = Get ready
telepronter-muy-corta = That reading was too short
telepronter-sin-micro = No microphone available
chat-telepronter-borrador = What is typed in the box
chat-telepronter-sin-texto = There is no text to read: type something in the box or attach a .txt or .md
chat-telepronter-abierto = The teleprompter is already open
chat-telepronter-hecha = Reading saved as a voice note

# The capture stack: the little pile in the corner (icon and panel).
pila-titulo-una = 1 capture
pila-titulo-varias = { $cuantas } captures stacked
pila-copiar-elegidas = Copy selected
pila-copiar-todas = Copy all
pila-quitar = Remove
pila-copiadas = { $cuantas } captures copied: paste them with Ctrl+V

# The drawing editor's properties panel (Excalidraw's).
panel-trazo = Stroke
panel-fondo = Background
panel-relleno = Fill
panel-grosor = Stroke width
panel-estilo-trazo = Stroke style
panel-presion = Pressure
panel-trazo-a-mano = Sloppiness
panel-bordes = Edges
panel-tipo-flecha = Arrow type
panel-fuente = Font family
panel-tamano-fuente = Font size
panel-alineacion-texto = Text align
panel-puntas = Arrowheads
panel-opacidad = Opacity
panel-capas = Layers
panel-alinear = Align
panel-acciones = Actions
panel-colores = Colors
panel-tonos = Shades
panel-codigo-hex = Hex code

## The canvas paper (Excalidraw viewBackgroundColor)
fondo-lienzo = Canvas background
fondo-papeles-del-movil = Phone papers
fondo-lienzo-menu = Canvas background…

## Export and print the canvas (G1, G2, G3, F11)
exportar-menu = Export…	Ctrl+Shift+E
exportar-copiar-png = Copy as PNG	Ctrl+Shift+C
exportar-titulo = Export the canvas
exportar-que = What
exportar-todo = Whole canvas
exportar-seleccion = Selection
exportar-marcos = Each frame
exportar-escala = Scale
exportar-transparente = Transparent background
exportar-tipo-png = PNG image
exportar-tipo-svg = SVG drawing
exportar-tipo-pdf = PDF document
exportar-tipo-html = Single-file web page
exportar-vacio = There is nothing to export with this.
exportar-fallo = The canvas could not be exported.
exportar-hecho = Exported: { $nombre }
imprimir-menu = Print…	Ctrl+P
imprimir-vacio = There is nothing to print.
imprimir-fallo = The canvas could not be printed.
exportar-chat = Export…
imprimir-chat = Print…
# F11: imprimir con vista previa (el dialogo de Windows y el boton de la barra)
imprimir-pista = Print… (Ctrl+P)
imprimir-que = What to print
imprimir-que-marcos = Each frame on its own page
imprimir-que-todo = The whole canvas on one page
imprimir-que-seleccion = Only the selection
marca-elige = What do you mark it with? (1-9, 0) · Esc to cancel
marca-plantar = Click on the canvas where the mark goes · Esc cancels
hojita-borrar = Clear
hojita-insertar = Insert
hojita-pegar = As a pin

# The share sheet (compartir.rs): one for everything, like the phone's.
compartir-titulo = Share
compartir-menu = Share…	Ctrl+Shift+S
compartir-proyecto = Share…
compartir-lector = Share…  (Ctrl+Shift+S)
compartir-boton = Share
compartir-guardar = Save as…
compartir-copiar = Copy to the clipboard
compartir-wifi = Send over Wi-Fi
compartir-web = Web page
compartir-pdf = PDF
compartir-png = PNG
compartir-jpg = JPG
compartir-svg = SVG
compartir-original = Original
compartir-originales = Originals
compartir-editable = Editable
compartir-texto = Text
compartir-csv = CSV
compartir-paginas-de = What to include · { $marcadas } of { $total }
compartir-una-pagina = Which page
compartir-va-entero = Goes whole, as it is
compartir-todas = All
compartir-ninguna = None
compartir-preparando = Preparing…
compartir-marca-alguna = Tick some page
compartir-en-cuanto-este = As soon as it is ready
compartir-peso-ficheros = { $peso } · { $n } files
compartir-panel-abierto = Choose where to send it
compartir-guardado = Saved: { $nombre }
compartir-copiado = Copied to the clipboard
compartir-no-se-pudo = It could not be prepared
compartir-nada = There is nothing to share here.
compartir-coma-decimal = .
compartir-lienzo = Canvas
compartir-lienzo-entero = Whole canvas
compartir-marco = Frame
compartir-pagina = Page
compartir-anotada = With the notes
compartir-sin-nombre = Untitled
compartir-tipo-lienzo = Canvas
compartir-tipo-pagina = PDF page
compartir-tipo-foto = Photo
compartir-tipo-tabla = Table
compartir-tipo-miniapp = Mini-app
compartir-tipo-voz = Voice note
compartir-tipo-nota = Note

# Drawing tools: the ones shown in the canvas, the reader, pins and the
# screen annotator. Turning one off removes it from the bar and its key.
ajustes-pestana-herramientas = Tools
herramientas-lazo = Lasso
herramientas-rectangulo = Rectangle
herramientas-rombo = Diamond
herramientas-elipse = Ellipse
herramientas-flecha = Arrow
herramientas-flecha-codos = Elbow arrow
herramientas-flecha-libre = Freehand arrow
herramientas-linea = Line
herramientas-lapiz = Pen
herramientas-grafito = Pencil
herramientas-texto = Text
herramientas-borrador = Eraser
herramientas-resaltador = Highlighter
herramientas-foco = Spotlight
herramientas-lupa = Magnifier
herramientas-mosaico = Mosaic
herramientas-arco = Arc
herramientas-serie = Numbered steps
herramientas-punto = Labelled point
herramientas-cota = Dimension
herramientas-escalar = Calibrate scale
herramientas-escala-grafica = Scale bar
herramientas-marco = Frame
herramientas-relleno = Fill bucket
herramientas-recortar = Trim
herramientas-extender = Extend
herramientas-copiar-estilo = Copy style

## D10: a PowerPoint in the chat (diapositivas.rs)
diapositivas-titulo = Presentation
diapositivas-no-es = This is not a presentation
diapositivas-keynote = Keynote files can't be read directly: export to PowerPoint or PDF
diapositivas-sin-powerpoint = Presenting a PowerPoint needs PowerPoint installed
diapositivas-abrir-con-otra = Open it with another app?
diapositivas-preparando = Preparing the presentation with PowerPoint…
diapositivas-no-se-pudo = Could not prepare the presentation
diapositivas-no-esta = The presentation is no longer on this computer

## Chat: a page from a project, the link card, «Received from» and the inbox
chat-sin-paginas = No project has pages to attach
chat-recibido-de = Received from { $de }
chat-fecha-corta = { $mes } { $dia }
chat-enlace-no-abre = The link could not be opened

## D11: a spreadsheet in the chat is a book of tables (ventana_chat/libro.rs)
libro-titulo = Spreadsheet
libro-xls-antiguo = Old .xls files can't be read: open it and save it as .xlsx
libro-abrir-fuera = Open it with another app?
libro-no-es = The file isn't a valid spreadsheet
libro-sin-hojas = The workbook has no sheet with data

## D9: find in the document (buscador.rs, visor.rs, lector_pdf.rs)
buscar-pista = Find in document
buscar-cuenta = { $n } of { $total }
buscar-nada = No results
buscar-leyendo = Reading the text…
buscar-sin-texto = This PDF has no text (it's an image)
buscar-cifrado = Can't search an encrypted PDF

## D5: rename from the reader (renombrar_doc.rs)
renombrar-hecho = Now called "{ $nombre }"
renombrar-ya-existe = There's already a file called "{ $nombre }"
renombrar-no-se-pudo = Couldn't change the name

## Canvas: zone, laser, image, shapes, graph and present (F8, F12, F14, G5)
herramientas-zona = Zone (rounded copy)
herramientas-laser = Laser pointer
herramientas-imagen = Image
herramientas-figuras = Shapes
figuras-grafica = Graph of a function…
figuras-tabla-en-blanco = Blank table
figuras-pegar-tabla = Paste table (from Excel)
figuras-guardar-seleccion = Save selection as a shape…
figuras-quitar = Remove
figuras-nombre-titulo = Save as a shape
figuras-nombre = Name
figuras-nombre-ayuda = Enter: save · Esc: cancel
figuras-sin-tabla = The clipboard has no table
grafica-titulo = Graph of a function
grafica-formulas = y = (one curve per line)
grafica-x-desde = x from
grafica-x-hasta = x to
grafica-y-desde = y from
grafica-y-hasta = y to
grafica-escala = Pixels per unit
grafica-ayuda = Enter: insert · Shift+Enter: another curve · Tab: next field · Esc: cancel · piecewise: x^2 si x<0; 2x si x>=0
grafica-limites-mal = The limits make no sense: "from" must be less than "to"
grafica-sin-formula = Type a formula
grafica-formula-mal = Can't understand:
grafica-solo-x = It can only use x:
presentar-ayuda = ← → PgDn: next · B: black · J: laser · L: pen · Esc: exit
vista-mirando = View only · Alt+R: edit · F5: present
herramientas-cronograma = Timeline
cronograma-titulo = Timeline
cronograma-filas = Rows
cronograma-columnas = Scale columns
cronograma-fila = Row
cronograma-ayuda = Enter: apply · Tab: next field · Esc: cancel · drag the bars on the drawing
cronograma-cuentas-mal = Rows from 0 to 60 and columns from 1 to 40

## --- Voice: two languages, lyrics, teleprompter, pronounce, conversation, call (B6-B11) ---

ajustes-voz-segundo = Second language of voice notes
ajustes-voz-modo = With two languages
ajustes-voz-modo-cada-uno = Each part as it was said
ajustes-voz-modo-todo-en-uno = All in the first one
ajustes-voz-ninguno = None
ajustes-voz-ingles = English
ajustes-voz-espanol = Spanish
ajustes-voz-portugues = Portuguese
chat-letra-editar = Edit the lyrics
chat-letra-editando = The lyrics are already being edited
chat-letra-guardar = Save
chat-letra-guardada = Lyrics saved
chat-cancelar-caja = Cancel
chat-llamada-secreta = Secret call
telepronter-pausa = Pause
telepronter-seguir = Resume
telepronter-velocidad-muy-lenta = very slow
telepronter-velocidad-lenta = slow
telepronter-velocidad-normal = normal
telepronter-velocidad-rapida = fast
telepronter-velocidad-muy-rapida = very fast
pronunciar-titulo = Pronounce
pronunciar-manten = Hold the space bar (or the microphone) and speak
pronunciar-suelta = Release to hear yourself
pronunciar-otra-vez = Hold again to repeat it
pronunciar-guia = Guide
pronunciar-guia-texto = Write a text
pronunciar-guia-fichero = An image, a PDF or a note from this computer
pronunciar-sin-guia = No guide: write one, or bring an image, a PDF or a note
pronunciar-repetir = Hear again (R)
pronunciar-guardar = Save to the chat (Ctrl+G)
pronunciar-ya-guardada = Saved
pronunciar-guardado = Saved; it is being transcribed so you can see what was understood
pronunciar-idioma = Language you practise
pronunciar-idioma-ajustes = The usual one
pronunciar-oir-texto = Hear the text
pronunciar-sin-voz = Windows has no voice for that language
pronunciar-no-se-pudo = That could not be opened
pronunciar-toma = Last take: { $duracion }
pronunciar-pagina = Page { $pagina } of { $paginas }
pronunciar-abierto = Pronounce is already open
conversacion-titulo = Conversation
conversacion-explicacion = It records non-stop; each person presses their number (1 to 9) when they start talking, and 0 when nobody is. The transcript comes out with the names.
conversacion-cuantos = How many people are talking?
conversacion-persona = Person { $n }
conversacion-empezar = Start recording
conversacion-terminar = Finish and transcribe
conversacion-turnos = { $n } turns
conversacion-hablando = Talking…
conversacion-teclas = Press the number of who is talking · 0: nobody
conversacion-transcribiendo = Transcribing the conversation…
conversacion-abierta = The conversation is already open
llamada-titulo = Call
llamada-entrante = Incoming call
llamada-contestar = Answer
llamada-colgar = Hang up
llamada-altavoz = Speaker
llamada-perdida = Missed call

## PDF zone (E9): merge PDF pages into one canvas, as on the phone
fusionar-paginas = Merge into one canvas
fusionar-paginas-empieza = Putting the pages together in a canvas…
fusionar-paginas-hecho = Done: “{ $nombre }” is now in the project
fusionar-paginas-no = Those pages could not be merged

## Universe zone (batch 2: H2, H3, M1): the Cosmos theme outside the universe
ajustes-tema-cosmos = Cosmos theme (starry sky in the chat)

## Zona pines (tanda 2: C3, C4, L3): magic word, board, sheet and tools
pin-convertir-en = Convert to
pin-herramienta-temporizador = Timer
pin-herramienta-cronometro = Stopwatch
pin-herramienta-tareas = To-do list
pin-herramienta-contador = Counter
pin-herramienta-gastos = Expenses
pin-herramienta-pizarra = Board
pin-herramienta-ruleta = Roulette
pin-herramienta-lienzo = Canvas
pin-herramienta-hoja = Sheet
pin-herramienta-tabla = Table
pin-fondo-pizarra = Board background
pin-pizarra-blanca = White
pin-pizarra-negra = Black
pin-pizarra-azul = Slate blue
pin-pizarra-verde = Chalkboard green
pin-pauta-lisa = Plain
pin-pauta-cuadros = Grid
pin-pauta-rayas = Lines
pin-pauta-columnas = Columns
pin-pauta-puntos = Dots
pin-tiempo-cumplido = Time's up
pin-hoja-nombre = Sheet

## Window groups (H9) and Markdown note editor (H12)
bandeja-grupos-ventanas = Window groups…
chat-grupos-ventanas = Window groups
chat-adj-nota-md = Note
chat-editar-nota = Open in the note editor
grupos-nada-abierto = No open canvases, documents or notes to save
grupos-guardar = { $n ->
    [one] Save the open window as a group…
   *[other] Save the { $n } open windows as a group…
}
grupos-abrir = Open “{ $nombre }” · { $n }
grupos-borrar = Delete “{ $nombre }”
grupos-alguna-falta = Some window in the group can no longer be opened
grupos-sin-nombre = Group
grupos-nombre = Window group name
grupos-guardar-boton = Save
nota-md-nueva = New note
nota-md-sufijo = PixPin note
nota-md-negrita = Bold
nota-md-cursiva = Italic
nota-md-tachado = Strikethrough
nota-md-codigo = Code
nota-md-enlace = Link
nota-md-titulo1 = Heading
nota-md-titulo2 = Subheading
nota-md-titulo3 = Section
nota-md-lista = List
nota-md-numerada = Numbered list
nota-md-casilla = Task list
nota-md-marcar = Check or uncheck the task
nota-md-cita = Quote
nota-md-bloque = Code block
nota-md-raya = Divider
nota-md-formula = Formula
nota-md-cortar = Cut
nota-md-copiar = Copy
nota-md-pegar = Paste
nota-md-guardar = Save
nota-md-copia = Save a .md copy…
nota-md-tipo = Markdown note
nota-md-no-guardada = The note could not be saved. Close anyway and lose the changes?
nota-md-mayus = Shift
nota-md-intro = Enter

# H12, the new note editor window (header, toolbar, menus and tables)
nota-md-compartir = Share
nota-md-tabla = Table
nota-md-imagen = Image
nota-md-fecha = Today's date
nota-md-pista-barra = Type / in the note for more
nota-md-cambiar-titulo = Rename
nota-md-fila-encima = Insert row above
nota-md-fila-debajo = Insert row below
nota-md-quitar-fila = Delete row
nota-md-columna-izquierda = Insert column left
nota-md-columna-derecha = Insert column right
nota-md-quitar-columna = Delete column
nota-md-quitar-tabla = Delete table
nota-md-boton-fila-mas = + Row
nota-md-boton-fila-menos = − Row
nota-md-boton-columna-mas = + Column
nota-md-boton-columna-menos = − Column
nota-md-meses = Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec
## Note tables: merge and colour cells (H12)
nota-md-boton-combinar = Merge
nota-md-boton-color = Colour
nota-md-combinar-celdas = Merge cells
nota-md-separar-celdas = Unmerge cells
nota-md-color-fondo = Background colour
nota-md-color-letra = Text colour
nota-md-aplicar-a = Cells|Whole row|Whole column
nota-md-nombres-fondo = No colour|Red|Green|Blue|Yellow
nota-md-nombres-letra = Automatic|Red|Green|Blue|Orange
nota-md-pista-color = Whole rows and columns: right-click
nota-md-imagen-no-copiada = The image could not be copied next to the note.

## Note comments (H12, Sep 30): the panel on the right
nota-md-com-titulo = Comments
nota-md-com-comentar = Comment
nota-md-com-ninguno = No comments
nota-md-com-pista = Select some text and press Comment (Ctrl+Alt+M)
nota-md-com-sin-ancla = Their text is no longer in the note
nota-md-com-responder-pista = Reply…
nota-md-com-responder = Reply
nota-md-com-guardar = Save
nota-md-com-cancelar = Cancel
nota-md-com-editar = Edit
nota-md-com-borrar = Delete
nota-md-com-resuelto = resolved
nota-md-com-ver-resueltos = Show resolved
nota-md-com-ocultar-resueltos = Hide resolved
nota-md-com-editado = edited
nota-md-com-borrar-hilo = Delete the comment and its replies?

## Markdown notes: images and live pages (H12)
nota-md-pagina-viva = Page from a project
nota-md-enlace-hoja = Link to a sheet
nota-md-captura = Screenshot
nota-md-ver-grande = View full size
nota-md-abrir-hoja = Open the sheet
nota-md-foto-pequena = Small
nota-md-foto-mediana = Medium
nota-md-foto-grande = Large
nota-md-foto-columna = Column width
nota-md-quitar-foto = Remove image
nota-md-hoja-borrada = The sheet is gone: this is the last copy
nota-md-otros-proyectos = Other projects
nota-md-sin-hojas = There are no project sheets to add to the note yet.
nota-md-solo-imagenes = Only images can be dropped here.
chat-insertar-en-nota = Insert into a note

## Canvas: weld vertices and number points (lienzo-geometria, 26-sep)
herramientas-nudo = Weld vertices
herramientas-bolita = Ball: sweep over to select
lienzo-nudo = Weld vertices
lienzo-nudo-ayuda = Click where two shapes touch and they are pinned together: they no longer come apart and, with a single pin, they turn around it. Click the pin to remove it; drag it with the hand to take the pinned shapes along.
lienzo-punto-serie = Point labels
lienzo-cota-pedir = Ask for the length when drawn
lienzo-cota-titulo = How long and which way? (Tab switches field)
lienzo-cota-largo = Length
lienzo-cota-angulo = Angle

## Canvas: image (zone, magnifier, pixelate, share and reference; Sep 26)
lienzo-referencia-menu = Reference image…
lienzo-pista-compartir = Share… (Ctrl+Shift+S)
lienzo-pista-lupa = Magnifier (Q): on the canvas, tap a closed shape to show what lies beneath, enlarged; on screen, zooms under the cursor
lienzo-pista-mosaico = Pixelate (mosaic): drag over what you want to hide
lienzo-pista-zona = Zone (Z): drag a box to get a cropped copy you can move
lienzo-panel-mosaico = Cover
lienzo-panel-aumento = Magnification
lienzo-panel-guia = Pointer
# The spotlight (Propiedad.OSCURECER and Propiedad.ZONA on the phone)
lienzo-panel-oscurecer = Darken
lienzo-panel-zona-foco = Lit area
lienzo-pista-foco = Spotlight (F): tap a closed shape and the area around it darkens, inside its frame
lienzo-pista-serie = Numbered steps: each click places the next number

## Projects screen in the chat and the Chat / Projects switch (ui/Proyectos.kt on the phone)
proyectos-interruptor-chat = Chat
proyectos-interruptor-proyectos = Projects
proyectos-hojas = { $total } sheets
proyectos-hojas-de = { $anotadas } of { $total } sheets annotated
proyectos-chat = Chat
proyectos-hoja = Sheet
proyectos-nota = Note
proyectos-tabla = Table
proyectos-hoja-hecha = New sheet in “{ $nombre }”
proyectos-renombrar = Rename
proyectos-renombrar-titulo = Project name
proyectos-guardar = Save
proyectos-archivar = Archive
proyectos-desarchivar = Unarchive
proyectos-compartir = Share
proyectos-copias = Backups
proyectos-borrar = Delete
proyectos-borrar-titulo = Delete “{ $nombre }”?
proyectos-borrar-aviso = It will also be deleted on your other devices the next time you sync. A copy stays in the project menu → “Backups” in case you change your mind.

## --- Canvas: graph and table dialogs, put into a cell (F12/F14) ---
cajetin-cancelar = Cancel
grafica-insertar = Insert
figuras-guardar = Save
figuras-editar-tabla = Edit table… (Enter)
figuras-meter-en-celda = Put the selection into its cell
tabla-blanco-titulo = Blank table
tabla-pegada-titulo = Pasted table
tabla-editar-titulo = Edit table
tabla-insertar = Insert
tabla-aplicar = Apply
tabla-mas-fila = + Row
tabla-menos-fila = − Row
tabla-mas-columna = + Column
tabla-menos-columna = − Column
tabla-cabecera = Header
tabla-pegar = Paste
tabla-ayuda = Tab and arrows: another cell · − removes the active cell's row or column · Enter: accept · Esc: cancel

## --- Canvas: the Zone to the project chat (F8, mandarLaZona on the phone) ---
zona-arrastra-copia = Drag: a copy comes out
zona-arrastra-chat = Drag: it goes to the project chat
zona-al-chat = To chat
zona-mandada = Zone sent to the «{ $proyecto }» chat
zona-no-mandada = The zone could not be sent
zona-viene-pdf = PDF «{ $proyecto }» → page { $pagina }
zona-viene-lienzo = Canvas «{ $nombre }»
zona-nombre-pagina = Zone of page { $pagina }
zona-nombre-lienzo = Zone of { $nombre }
zona-lienzo = canvas
zona-hoja-lienzo = Canvas
# Linked zone that leads to no canvas (salto_por_enlace).
zona-enlace-roto = This zone does not lead to any canvas in this project

## The canvas toolbar grouped like the phone's (GRUPOS_DE_FABRICA)
herramientas-mano = Select and move
herramientas-imprimir = Print
herramientas-compartir = Share
barra-grupo-elegir = Other ways to select
barra-grupo-trazar = Freehand
barra-grupo-formas = Shapes
barra-grupo-flechas = Arrows and lines
barra-grupo-arreglar = Fix what is drawn
barra-grupo-nombrar = Text and labels
barra-grupo-tapar = Cover and point
barra-grupo-medir = Measure
barra-grupo-laminas = Tables, charts and timeline
barra-grupo-marco = Frame and image
barra-grupo-sacar = Share and print
barra-pista-mano = Select and move (M)
barra-pista-deshacer = Undo (Ctrl+Z)
barra-pista-rehacer = Redo (Ctrl+Y)
barra-pista-salir = Exit (Esc)
ajustes-herramientas-sueltas = Always visible

## PDF reader: «To project» (the notes move to a project's sheets and travel when syncing)
lector-al-proyecto = To project
lector-al-proyecto-hecho = It's in projects now, with your notes
lector-al-proyecto-fallo = The project could not be created

## Word and book reader: the four types and four weights of the phone (K16)
lector-letra-serif = Serif
lector-letra-sans = Sans
lector-letra-cursiva = Cursive
lector-grosor-fina = Light
lector-grosor-negra = Black

## Screen annotator: its pill (CapaPantalla.kt on the phone) and saving to «Saved messages»
anotador-rotulo-dibujando = Alt + double middle click: click through
anotador-rotulo-atravesando = Alt + double middle click: draw again
anotador-globo-titulo = Annotated screen
anotador-globo-guardado = Saved to Saved messages
anotador-globo-fallo = Could not save to Saved messages
anotador-globo-copiado = Copied to the clipboard
# Screen annotator: click-through in the toolbar (2026-09-29)
anotador-boton-atravesar = Click through
anotador-pista-atravesar = Click through (Space): the mouse goes to what is underneath and the drawing stays visible. Come back with the hand on the pill or with Alt + middle double-click

# Share sheet: a document PDF with its annotations or clean (2026-09-29)
compartir-con-anotaciones = With annotations
compartir-con-anotaciones-detalle = Ink, margins and bookmarks on top of the PDF; off, the clean PDF

# Share sheet: a PDF web page as lines or as images (2026-09-30, Android v0.98.1)
compartir-texto-buscable = Searchable text
compartir-texto-buscable-detalle = Pages as lines with their text; off, as images

# Secret call: who is calling and call back later (2026-09-30, Android v0.98.6)
chat-quien-llama = Who is calling: { $nombre }
chat-quien-llama-barra = Who is calling:
chat-quien-llama-teclas = Enter: pick the time · Esc leaves it
llamada-volver-en = Call back in
llamada-vuelve-a-las = It will call you back at { $hora }

## H12: editing without marks, the floating toolbar and the note font (Sep 30)
nota-md-texto-normal = Normal text
nota-md-quitar-formato = Clear formatting
nota-md-enlace-pista = Paste or type a link and press Enter
nota-md-emojis = Smile|Laugh|Love it|Fine|Like|Dislike|Thanks|Applause|Party|Done|No|Warning|Heart|Fire|Star|Idea
nota-md-letra-texto = Text font
nota-md-letra-titulos = Heading font
nota-md-tamano = Font size
nota-md-tamanos = Small|Normal|Large|Extra large
nota-md-solo-esta-nota = Only in this note
nota-md-pista-tamano = Ctrl + wheel changes it one step at a time

# --- Notes: embedded documents, chat messages and audio (H12, Oct 1) ---
nota-md-documento = Document
nota-md-del-chat = From the chat
nota-md-audio = Audio
nota-md-abrir = Open
nota-md-quitar = Remove from the note
nota-md-ir-al-mensaje = Go to the message in the chat
nota-md-mensaje-borrado = Deleted message
nota-md-no-esta = Not on this computer
nota-md-pasar-a-texto = Transcribe
nota-md-pasando-a-texto = Transcribing { $pct } %
nota-md-sin-mensajes = This project's chat has no messages to insert (a standalone note has no chat).
nota-md-nota-de-voz = Voice note

## Export a note to Word (H12, 1-oct)
nota-md-exportar-word = Export to Word…
nota-md-tipo-word = Word document
compartir-word = Word

## Requests from other programs (Flow Launcher, scripts; docs/protocolo-pedidos.md)
pedido-tareas = Tasks
pedido-escrito = Written in “{ $proyecto }”.
microfono-abriendo = Opening the microphone…
microfono-guardando = Saving…
microfono-parar = Click to stop · Esc discards it
microfono-guardada = Voice note saved
microfono-convertir = Turn into a call
microfono-listo = Done
microfono-cuando = What time should I call you?
microfono-en-15-min = In 15 min
microfono-en-1-hora = In 1 h
microfono-manana = Tomorrow 9:00
microfono-poner = Set the call
microfono-volver = Back
microfono-sonara = It will call you: { $cuando }
microfono-puesta = Done: it will call you { $cuando }
microfono-teclas = Type the time (18:30) or use the arrows · Enter sets it
soltar-aqui = Drop here to add to “{ $proyecto }”
soltar-anadiendo = Adding…
soltar-anadido-uno = 1 added to the chat
soltar-anadidos = { $n } added to the chat
soltar-ninguno = Nothing could be added
chat-anadir-arrastrando = Add by dragging…
pedido-lista-hecha = List “{ $lista }” created in “{ $proyecto }”.
pedido-tarea-anadida = “{ $tarea }” added to “{ $lista }”.
pedido-tarea-hecha = “{ $tarea }” marked as done.
pedido-tarea-pendiente = “{ $tarea }” is pending again.
pedido-roto = A request from another program was missing something; nothing was done.
pedido-sin-proyecto = That project is no longer in PixPin.
pedido-sin-mensaje = That message is no longer in the chat.
pedido-no-es-lista = That message isn't a to-do list.
pedido-sin-tarea = The list has no task number { $indice }.
pedido-tarea-vacia = A task with no text isn't added.
pedido-sin-fichero = That file is no longer on disk.
pedido-fallo-disco = What was asked couldn't be saved. Check PixPin's log.
pedido-tarea-movida = “{ $tarea }” moved to “{ $lista }”.
pedido-tarea-cambio = The list changed while the task was being moved; nothing was touched. Look it up again.
pedido-sin-capturas = There are no captures yet.
pedido-fuera-de-capturas = That file isn't in the captures folder; it's left alone.
pedido-no-es-imagen = That file isn't an image.
pedido-imagen-no-se-pudo = The image couldn't be copied: { $motivo }
pedido-sin-imagen = The image { $ruta } can't be found; the task wasn't added.

## Capture gallery (2-oct)
galeria-titulo = Captures
galeria-titulo-cuantas = Captures · { $cuantas }
galeria-vacia = No captures yet. The ones you copy with Alt + drag will show up here.
galeria-abrir-carpeta = Open folder
galeria-copiada = Copied to the clipboard
galeria-pineada = Pinned
galeria-borrada = Moved to PixPin's recycle bin
galeria-no-se-pudo = Couldn't do it: { $motivo }
galeria-caducan = They delete themselves after 7 days (to the Windows recycle bin). Keep saves them in the chat.
galeria-se-borra = Deleted on { $fecha }
galeria-se-borra-hoy = Deleted today
galeria-conservada = Kept
galeria-conservada-aviso = Kept: it is in Saved messages and won't be deleted
bandeja-galeria-capturas = Capture gallery…
chat-galeria-capturas = Capture gallery
# Lessons learned (3-oct, as on the phone)
chat-lecciones = Lessons
chat-leccion-nueva = New lesson
chat-hacer-leccion = Make a lesson
bandeja-lecciones = 💡 Lessons…
bandeja-leccion-nueva = New lesson…
bandeja-tareas = ☑ Tasks…

## Tareas (ventana)
tareas-titulo = Tasks
tareas-resumen = { $pendientes ->
    [0] Nothing pending
    [one] 1 pending
   *[other] { $pendientes } pending
  } · { $listas ->
    [one] 1 list
   *[other] { $listas } lists
  }
tareas-apuntar = Write a task for the Inbox and press Enter (Ctrl+V pastes images)
tareas-apuntar-boton = Add
tareas-hechas = Done ({ $n })
tareas-vacia = No tasks yet. Type above and press Enter: it goes to the Inbox, and from there you move it to its group.
tareas-cambio = The list changed in the meantime; it's up to date now. Tick it again.
tareas-mover = Move to…
tareas-mover-titulo = Move “{ $tarea }” to…
tareas-sin-grupos = There are no other task groups yet. Create a task list in a chat and it will show up here.
tareas-movida = Moved to { $lista }
tareas-imagen-no-esta = That image isn't on this computer yet: it will arrive with the sync.
tareas-pegar-no-imagen = Only images or text can be pasted into a task.
tareas-pegar-repetida = That image is already in as { $ficha }.

# Opening a photo that Windows can only read with a Microsoft Store extension.
abrir-falta-extension = Can't open { $nombre }: Windows needs "{ $extension }", free in the Microsoft Store. Install it and open the file again.

## v2-menus
v2menus-responder = Reply
v2menus-anotar-encima = Annotate on top
v2menus-mas = More
v2menus-elegir-varios = Select several
v2menus-tecla-supr = Del
v2menus-tecla-clic = click
v2menus-adj-buscar = Search: table, stopwatch, PDF…
v2menus-adj-mas = MORE
v2menus-adj-nada = Nothing matches your search
v2menus-adj-lienzo = Canvas
v2menus-adj-tareas = To-do list
v2menus-adj-tabla = Table
v2menus-adj-conversacion = Conversation
v2menus-adj-captura = Screenshot
v2menus-adj-pagina = A project page
v2menus-adj-proyecto = Whole project
v2menus-adj-movil = Bring from phone
v2menus-envio-enviar = Send
v2menus-envio-anadir = Add
v2menus-envio-fotos = { $cuantos ->
    [one] 1 photo
   *[other] { $cuantos } photos
} · to { $proyecto }
v2menus-envio-archivos = { $cuantos ->
    [one] 1 file
   *[other] { $cuantos } files
} · to { $proyecto }
## v2-ajustes
ajustes2-titulo = Settings
ajustes2-buscar = Search settings…
ajustes2-nota = Saved when you close with Done or Esc.
ajustes2-nota-punto = = different from the default.
ajustes2-deshacer = Undo
ajustes2-listo = Done
ajustes2-restablecer = Reset { $seccion }
ajustes2-resultados = Results for “{ $busqueda }”
ajustes2-resultados-sub = { $n ->
    [one] 1 setting found across all sections.
   *[other] { $n } settings found across all sections.
  }
ajustes2-sin-resultados = No setting mentions “{ $busqueda }”. Try another word.
ajustes2-sec-general = General
ajustes2-sec-general-sub = How PixPin starts and what it does with PDFs in the chat.
ajustes2-sec-captura = Capture
ajustes2-sec-captura-sub = How a capture is taken and what happens to it afterwards.
ajustes2-sec-lienzo = Canvas and tools
ajustes2-sec-lienzo-sub = The magnet, freehand strokes and which tools show in the toolbar.
ajustes2-sec-voz = Voice and transcription
ajustes2-sec-voz-sub = Turning voice notes with two languages into text.
ajustes2-sec-atajos = Shortcuts
ajustes2-sec-atajos-sub = Click a box, then press the keys. Delete leaves it without a shortcut; Esc cancels.
ajustes2-sec-sincro = Sync
ajustes2-sec-sincro-sub = How this computer finds the phone.
ajustes2-sec-apps = Default apps
ajustes2-sec-apps-sub = Open images and videos with PixPin from File Explorer.
ajustes2-sec-apariencia = Appearance
ajustes2-sec-apariencia-sub = How the chat looks.
ajustes2-sec-avanzado = Advanced
ajustes2-sec-avanzado-sub = Performance, the pen filter and the settings file.
ajustes2-g-arranque = On startup
ajustes2-g-pdf = PDF documents
ajustes2-g-al-capturar = When capturing
ajustes2-g-gif = GIF recording
ajustes2-g-guardar = Saving and cleanup
ajustes2-g-pila = The capture stack
ajustes2-g-ignorados = Ignored programs
ajustes2-g-regiones = Saved regions
ajustes2-g-iman = Magnet
ajustes2-g-trazo = Freehand
ajustes2-g-voz = Voice notes
ajustes2-g-atajos = Global shortcuts
ajustes2-g-sincro = On the local network
ajustes2-g-apps = Opening images and videos
ajustes2-g-tema = Theme
ajustes2-g-rendimiento = Performance
ajustes2-g-lapiz = Pen filter (Natural mode)
ajustes2-g-fichero = The settings file
ajustes2-ayuda-arranque = Stays in the tray without opening any window. Not available in portable mode.
ajustes2-ayuda-idioma = Applies from the next start.
ajustes2-ayuda-pdf-aligerar = After it enters the chat, in the background.
ajustes2-pdf-nivel = How much lighter
ajustes2-ayuda-pdf-nivel = On entry only Exact and Medium are used; Small and Max. are for “Make PDF lighter” by hand.
ajustes2-retardo = Delayed capture
ajustes2-ayuda-retardo = Countdown to open a menu or a tooltip before capturing.
ajustes2-color = Color format
ajustes2-ayuda-color = How the eyedropper copies the color.
ajustes2-scroll = Maximum scrolling height
ajustes2-ayuda-scroll = Limit for long captures, so an endless page does not use up memory.
ajustes2-gif-ritmo = Frames per second
ajustes2-ayuda-gif-ritmo = Fewer = lighter file. 10 or 15 is enough for tutorials.
ajustes2-gif-retardo = Delay before recording
ajustes2-ayuda-gif-retardo = Seconds between pressing “Record” and the first frame.
ajustes2-carpeta = Captures folder
ajustes2-abrir = Open
ajustes2-caducidad = Days until deleted
ajustes2-ayuda-caducidad = After this they go to the Windows Recycle Bin, except the kept ones.
ajustes2-apilar = Stack in the corner
ajustes2-ayuda-apilar = Each capture leaves a box in the corner; click it to gather the next ones and paste them all with one Ctrl+V.
ajustes2-esquina = Box corner
ajustes2-ayuda-esquina = On the monitor where the capture was taken.
ajustes2-icono = Time on screen
ajustes2-ayuda-icono = How long the unarmed box stays after the last capture.
ajustes2-ayuda-ignorado = While this one is in front, PixPin shortcuts do nothing.
ajustes2-anadir-programa = Add a program
ajustes2-ayuda-anadir-programa = Type the executable name and press Enter.
ajustes2-anadir = Add
ajustes2-ayuda-region = { $x }, { $y } · { $ancho } × { $alto } px
ajustes2-ayuda-iman = While drawing, the cursor snaps to what is already there.
ajustes2-ayuda-iman-esquinas = Shape corners and line ends.
ajustes2-ayuda-iman-medios = The midpoint of each side.
ajustes2-ayuda-iman-centros = The center of each shape.
ajustes2-iman-radio = Snapping distance
ajustes2-ayuda-iman-radio = In screen pixels.
ajustes2-suavizado = Stroke smoothing
ajustes2-ayuda-suavizado = Natural removes the shake of a still hand without delaying the tip.
ajustes2-ayuda-voz-segundo = Whisper also understands this language inside the same note.
ajustes2-ayuda-voz-modo = What to do with what is said in the second language.
ajustes2-presencia = Let the phone find me
ajustes2-ayuda-presencia = The phone sees this computer as soon as it opens Sync. Off, no port is opened on the network.
ajustes2-lo-mio = Mine wins
ajustes2-ayuda-lo-mio = When syncing, this computer wins: only what does not exist here comes from the other side.
ajustes2-abrir-con = Offer PixPin for images and videos
ajustes2-ayuda-abrir-con = Shows in “Open with” and in Default apps. Turning it off removes what was written to the registry.
ajustes2-predeterminada = Default app
ajustes2-ayuda-predeterminada = Windows only lets you choose it in Settings: there, PixPin Max → Set default.
ajustes2-hacer-predeterminada = Make PixPin the default app…
ajustes2-ayuda-cosmos = Starry sky behind the chat project list. Applies from the next chat.
ajustes2-nivel = Performance level
ajustes2-ayuda-nivel = Automatic measures the computer at startup. Applies from the next start.
ajustes2-medir = Measure editor frames
ajustes2-ayuda-medir = Logs how long each frame takes. Only for diagnostics.
ajustes2-ritmo = Paint right before the refresh
ajustes2-ayuda-ritmo = The pen tip lags less; a missed calculation loses one refresh.
ajustes2-paneo = Scroll without repainting
ajustes2-ayuda-paneo = The Windows compositor moves and zooms the canvas. Off, every frame is repainted.
ajustes2-corte = Cutoff with the pen still
ajustes2-ayuda-corte = Lower = less shake and more lag when starting. Auto = 1 Hz.
ajustes2-beta = How much it follows speed
ajustes2-ayuda-beta = Higher = tip closer to the cursor and more shake. Auto = 0.007.
ajustes2-fichero = Settings file
ajustes2-ayuda-fichero = Whatever is not here is still there, with its comments. Hand edits apply after a restart.
ajustes2-abrir-fichero = Open the file
ajustes2-u-s = { $n } s
ajustes2-u-px = { $n } px
ajustes2-u-dias = { $n ->
    [one] 1 day
   *[other] { $n } days
  }
ajustes2-u-hz = { $n } Hz
ajustes2-nunca = Never
ajustes2-siempre = Always
ajustes2-auto = Auto
