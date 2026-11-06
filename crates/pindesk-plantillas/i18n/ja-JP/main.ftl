# PinDesk Plantillas: UI strings (English, the reference together with es-ES).
# Rules: docs/idiomas.md. Variables ({ $n }) are the same in every language.

## The product
# The working name: the same in every language (see NOMBRE_PRODUCTO).
plt-producto = PinDesk Plantillas

## The top bar
plt-nuevo = New
plt-abrir = Open
plt-guardar = Save
plt-guardar-como = Save as…
plt-exportar = Export
plt-exportar-pdf = PDF
plt-exportar-png = PNG
plt-exportar-svg = SVG
plt-exportar-lienzo = Canvas (.excalidraw)
plt-enviar-pindesk = Send to PinDesk
plt-enviada = Sent: it is now in PinDesk Figures
plt-galeria = Gallery
plt-deshacer = Undo
plt-rehacer = Redo
plt-dia-noche = Day or night
plt-cerrar = Close

## The gallery
plt-galeria-titulo = Start from a template
plt-galeria-vacia = Blank
plt-gal-mensual = Monthly calendar
plt-gal-diario = Daily journal
plt-gal-semanal = Weekly planner
plt-gal-dia = Daily planner
plt-gal-tareas = To-do list
plt-gal-habitos = Habit tracker
plt-gal-cornell = Cornell notes
plt-gal-reunion = Meeting notes
plt-gal-puntos = Dot grid notebook

## Pages
plt-paginas = { $n ->
    [one] { $n } page
   *[other] { $n } pages
}
plt-pagina-de = Page { $n } of { $total }
plt-anterior = Previous
plt-siguiente = Next
plt-zoom-encajar = Fit

## The panels
plt-bloques = Blocks
plt-arrastra = Drag onto the page
plt-propiedades = Properties
plt-documento = Document
plt-sin-seleccion = Pick a block or drag one from the left
plt-editar-texto = Type and press Enter
plt-nombre = Name
plt-sin-titulo = Untitled
plt-papel = Paper
plt-horizontal = Landscape
plt-margenes = Margins (mm)
plt-tema = Theme
plt-idioma-fechas = Date language
plt-repetir = One page per
plt-desde = From
plt-hasta = To
plt-sin-finde = Skip weekends
plt-duplicar = Duplicate
plt-borrar = Delete
plt-subir = Move up
plt-bajar = Move down
plt-envolver-fila = Wrap in a row
plt-envolver-columna = Wrap in a column

## Palette groups
plt-grupo-estructura = Layout
plt-grupo-texto = Text and notes
plt-grupo-listas = Lists and tables
plt-grupo-fechas = Dates and calendars
plt-grupo-especiales = Specials

## Blocks
plt-pieza-fila = Row
plt-pieza-columna = Column
plt-pieza-rejilla = Grid
plt-pieza-seccion = Section
plt-pieza-pila = Layers
plt-pieza-encabezado = Heading
plt-pieza-texto = Text
plt-pieza-notas = Notes area
plt-pieza-lineas = Ruled lines
plt-pieza-puntos = Dots
plt-pieza-cuadricula = Grid paper
plt-pieza-divisor = Divider
plt-pieza-espacio = Spacer
plt-pieza-imagen = Image
plt-pieza-vinetas = Bullets
plt-pieza-casillas = Checkboxes
plt-pieza-numerada = Numbered list
plt-pieza-tabla = Table
plt-pieza-campo = Date field
plt-pieza-campo_fecha = Date
plt-pieza-campo_dia_semana = Weekday
plt-pieza-campo_semana = Week number
plt-pieza-campo_mes = Month
plt-pieza-campo_pagina = Page number
plt-pieza-calendario_mes = Month calendar
plt-pieza-calendario_mini = Mini calendar
plt-pieza-calendario_semana = Week
plt-pieza-horario = Schedule
plt-pieza-habitos = Habits
plt-pieza-animo = Mood
plt-pieza-prioridades = Priorities
plt-pieza-cornell = Cornell notes
plt-pieza-reunion = Meeting

## Properties
plt-prop-tam = Size
plt-prop-tam_mm = Millimetres
plt-prop-tam_fr = Parts
plt-prop-hueco = Gap
plt-prop-columnas = Columns
plt-prop-titulo = Title
plt-prop-recuadro = Border
plt-prop-texto = Text
plt-prop-nivel = Level
plt-prop-alineacion = Alignment
plt-prop-puntos = Font size
plt-prop-pauta = Ruling
plt-prop-paso = Step
plt-prop-items = Items
plt-prop-vacias = Empty lines
plt-prop-filas = Rows
plt-prop-cabeceras = Headers
plt-prop-trazo = Stroke
plt-prop-etiqueta = Label
plt-prop-campo = Value
plt-prop-formato = Format
plt-prop-antes = Before
plt-prop-despues = After
plt-prop-negrita = Bold
plt-prop-mes = Fixed month (YYYY-MM)
plt-prop-mini = Small
plt-prop-fin_de_semana = Include weekend
plt-prop-desde = From
plt-prop-hasta = Until
plt-prop-dias = Days
plt-prop-habitos = Habits
plt-prop-niveles = Levels
plt-prop-etiquetas = Labels

## Options
plt-opcion-auto = Automatic
plt-opcion-mm = Fixed
plt-opcion-fr = Share
plt-opcion-izquierda = Left
plt-opcion-centro = Centre
plt-opcion-derecha = Right
plt-opcion-rayado = Lines
plt-opcion-puntos = Dots
plt-opcion-cuadricula = Grid
plt-opcion-blanco = Blank
plt-opcion-continuo = Solid
plt-opcion-discontinuo = Dashed
plt-opcion-punteado = Dotted
plt-opcion-acento = Accent
plt-opcion-fecha = Date
plt-opcion-dia = Day
plt-opcion-dia_semana = Weekday
plt-opcion-semana = Week
plt-opcion-mes = Month
plt-opcion-ano = Year
plt-opcion-pagina = Page
plt-opcion-corto = Short
plt-opcion-largo = Long
plt-opcion-completo = Full
plt-opcion-iso = ISO
plt-opcion-numero = Number
plt-opcion-abreviado = Abbreviated
plt-opcion-letra = Initial

## Paper, themes and repetition
plt-papel-a4 = A4
plt-papel-a5 = A5
plt-papel-carta = Letter
plt-papel-tableta = Tablet
plt-papel-pantalla = Screen 16:9
plt-papel-medida = Custom
plt-tema-planificacion = Planning
plt-tema-academico = Academic
plt-tema-creativo = Creative
plt-tema-diario = Journal
plt-tema-minimo = Minimal
plt-tema-noche = Night
plt-tema-propio = Custom
plt-cada-ninguna = Just one
plt-cada-dia = Day
plt-cada-semana = Week
plt-cada-mes = Month

## AI
plt-ia-describir = Describe it
plt-ia-pista = For example: weekly planner with a 7 to 22 schedule, a to-do list and habits
plt-ia-crear = Create
plt-ia-pensando = Thinking…
plt-ia-vista-previa = AI preview
plt-ia-aplicar = Apply
plt-ia-descartar = Discard
plt-ia-ajustar = Adjust the selection
plt-ia-ajustar-pista = For example: more rows, put it in two columns
plt-ia-sugerir = Suggest a fill
plt-ia-sin-clave = No AI key: made without a model. Add your Claude or DeepSeek key in PinDesk Settings.
plt-ia-error = The AI could not do it: { $motivo }
plt-ia-avisos = { $n ->
    [one] { $n } thing in the answer was fixed
   *[other] { $n } things in the answer were fixed
}

## Notices
plt-guardado = Saved
plt-exportado = Exported: { $ruta }
plt-error-abrir = Could not open { $ruta }
plt-error-guardar = Could not save: { $motivo }
plt-cambios-sin-guardar = There are unsaved changes. Save them first?
plt-filtro = PinDesk template

## What a new block contains
plt-def-seccion = Section
plt-def-titulo = Title
plt-def-texto = Write here
plt-def-notas = Notes
plt-def-imagen = Image
plt-def-habitos = Water, Read, Exercise, Sleep 8 h
plt-def-habitos-mas = Meditate, No screens, Walk, Fruit
plt-def-animo = How do I feel?
plt-def-prioridades = High, Medium, Low
plt-def-prioridades-titulo = Priorities
plt-def-cornell = Cues, Notes, Summary
plt-def-reunion = Meeting
plt-def-reunion-rotulos = Date, Attendees, Agenda, Decisions, Action items
plt-def-tareas = Tasks
plt-def-metas = Goals
plt-def-gratitud = I am grateful for
plt-def-hoy = Today
plt-def-manana = For tomorrow
plt-def-semana = Week
plt-def-importante = Important
plt-def-cuando = When I can
