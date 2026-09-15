# La interfaz de Excalidraw, medida para copiarla en PixPin

Inventario documental de la interfaz de escritorio de Excalidraw (MIT) para que el
lienzo de PixPin (`apps/pixpin/src/ventana_editor.rs`, `crates/pixpin-ui`) la copie
con Direct2D. **Nada inventado:** cada dato lleva su origen `fichero:línea`.

## Convenciones

- `EX/` = clon disperso de `excalidraw/excalidraw`, commit **4e758db**
  (`C:\Users\MaxBook\AppData\Local\Temp\claude\F--THE-FORGE-PIXPIN-PC-VERSION-MAX\a9947ec1-5837-4b08-a9f3-7c7db700741c\scratchpad\excalidraw`).
  Rutas abreviadas: `C/` = `EX/packages/excalidraw/components/`, `CSS/` =
  `EX/packages/excalidraw/css/`, `ACT/` = `EX/packages/excalidraw/actions/`,
  `COM/` = `EX/packages/common/src/`, `ELE/` = `EX/packages/element/src/`,
  `LOC/` = `EX/packages/excalidraw/locales/`.
- Rutas de PixPin: relativas a la raíz del repo.
- **px**: el paquete trabaja en `rem` y no fija `font-size` en `:root` (buscado en
  `CSS/*.scss`), así que se convierte con 1 rem = 16 px, el valor por defecto del
  navegador. Cuando un número es **derivado** (una suma hecha aquí, no escrita en el
  código) se marca «(derivado)».
- Etiquetas: clave i18n → inglés (`LOC/en.json:línea`) / español (`LOC/es-ES.json:línea`).
  «—» = la clave no existe en `es-ES.json`; «""» = existe pero vacía (Excalidraw cae
  al inglés).

---

## 1. Disposición en escritorio

### 1.1 Rejilla general

- Contenedor superior `FixedSideContainer side="top"` (`C/LayerUI.tsx:313`), pegado a
  `--editor-container-padding` = 1rem = **16 px** por los cuatro lados
  (`C/FixedSideContainer.scss:13-18`, `CSS/theme.scss:51`).
- Dentro, `.App-menu_top` es una rejilla de **3 columnas `1fr 2fr 1fr`** con hueco
  **1rem (16 px)**; a partir de 1536 px de ancho pasa a `1fr 1fr 1fr` y hueco 3rem
  (`CSS/styles.scss:388-403`).
  - Columna 1 `App-menu_top__left`: menú hamburguesa y, debajo, panel de propiedades
    (`C/LayerUI.tsx:315-348`, `CSS/styles.scss:405-407`).
  - Columna 2 `shapes-section`: barra de herramientas centrada
    (`C/LayerUI.tsx:352-398`, `CSS/styles.scss:378-386`, `437-439`).
  - Columna 3 `layer-ui__wrapper__top-right`: botón de biblioteca (y lista de
    colaboradores/estadísticas si procede), alineado a la derecha, hueco 0.75rem
    (`C/LayerUI.tsx:400-436`, `C/LayerUI.scss:19-33`, `CSS/styles.scss:449-452`).
- Pie `footer.App-menu_bottom`: `position:absolute; bottom:1rem; padding:0 1rem;
  justify-content:space-between` (`C/footer/Footer.tsx:33-36`,
  `CSS/styles.scss:454-474`). Izquierda: zoom + deshacer/rehacer; derecha: ayuda.
- Espaciados de `LayerUI` (modo completo): `menuTopGap 6`, `toolbarColGap 4`,
  `toolbarRowGap 1`, `toolbarInnerRowGap 1`, `islandPadding 1`
  (`C/LayerUI.tsx:187-194`). Todos se multiplican por `--space-factor` = 0.25rem
  (`C/Stack.scss:2-6`, `CSS/theme.scss:80`): hamburguesa→panel **24 px**.
- `Stack.Row` / `Stack.Col` son `display:grid` con `gap: calc(--space-factor * --gap)`
  (`C/Stack.scss:1-19`, `C/Stack.tsx:24`, `46`).

### 1.2 Barra de herramientas (arriba, centro)

Es un `Island padding={1}` con clase `App-toolbar` (`C/Toolbar.tsx:247-253`, líneas de
`Toolbar.tsx` propias) y dentro un `Stack.Row gap={1}` (4 px) (`C/Toolbar.tsx:262`).
Encima de ella flota `HintViewer` (texto de ayuda contextual, 0.75rem, color
`--color-gray-40`, `top:100%; margin-top:.5rem`) (`C/Toolbar.tsx:255-260`,
`C/HintViewer.scss:7-21`).

Orden exacto de izquierda a derecha (`C/Toolbar.tsx:262-323`):

| # | Elemento | Tecla (`C/Tools.tsx`) | Icono (`iconos.json`) | Etiqueta en / es |
|---|---|---|---|---|
| 0 | `PenModeButton` — **solo si hay lápiz detectado** (`C/PenModeButton.tsx:15-16`) | — | `PenModeIcon` | `toolBar.penMode` "Pen mode - prevent touch" / "Modo Lápiz - previene toque" (`en:334`/`es:330`) |
| 1 | **Candado** `LockButton` (`C/Toolbar.tsx:275-281`) | Q (`C/LockButton.tsx:22`) | `UnlockedIcon` / `LockedIcon` (`C/LockButton.tsx:19`) | `toolBar.lock` "Keep selected tool active after drawing" / "Mantener la herramienta seleccionada activa después de dibujar" (`en:333`/`es:329`) |
| — | Separador 1 px × 1.5rem, `margin-right:.25rem` (`C/Toolbar.tsx:283-286`, `C/Toolbar.scss:18-23`) | | | |
| 2 | Mano (sin distintivo de tecla: `hideKeyBinding`) (`C/Toolbar.tsx:290`) | H (`C/Tools.tsx:70-74`) | `handIcon` | `toolBar.hand` "Hand (panning tool)" / "Mano (herramienta de panorama)" (`en:344`/`es:339`) |
| 3 | Selección (o Lazo si es la preferida) (`C/Toolbar.tsx:291-297`) | V o 1 (`C/Tools.tsx:75-80`) | `SelectionIcon` (`LassoIcon`) | `toolBar.selection` "Selection" / "Selección" (`en:322`/`es:318`) |
| 4 | Rectángulo | R o 2 (`C/Tools.tsx:81-86`) | `RectangleIcon` | "Rectangle" / "Rectángulo" (`en:325`/`es:321`) |
| 5 | Rombo | D o 3 (`C/Tools.tsx:87-92`) | `DiamondIcon` | "Diamond" / "Diamante" (`en:326`/`es:322`) |
| 6 | Elipse | O o 4 (`C/Tools.tsx:93-98`) | `EllipseIcon` | "Ellipse" / "Elipse" (`en:327`/`es:323`) |
| 7 | Flecha | A o 5 (`C/Tools.tsx:99-104`) | `ArrowIcon` | "Arrow" / "Flecha" (`en:328`/`es:324`) |
| 8 | Línea | L o 6 (`C/Tools.tsx:105-110`) | `LineIcon` | "Line" / "Línea" (`en:329`/`es:325`) |
| 9 | Dibujo libre | P (también X) o 7 (`C/Tools.tsx:111-115`) | `FreedrawIcon` | "Draw" / "Dibujar" (`en:330`/`es:326`) |
| 10 | Texto | T o 8 (`C/Tools.tsx:116-120`) | `TextIcon` | "Text" / "Texto" (`en:331`/`es:327`) |
| 11 | Nota adhesiva | N (`C/Tools.tsx:121-124`) | `stickyNoteToolIcon` | "Sticky note" / — (`en:339`) |
| 12 | Borrador | E o 0 (`C/Tools.tsx:129-134`) | `EraserIcon` | "Eraser" / "Borrar" (`en:336`/`es:332`) |
| — | Separador 1 px, `margin-left:.25rem` (`C/Toolbar.tsx:312-315`) | | | |
| 13 | **Más herramientas** (desplegable) (`C/Toolbar.tsx:317-322`) | — | `DotsIcon`, o el icono de la herramienta extra activa (`C/Toolbar.tsx:102-116`) | `toolBar.extraTools` "More tools" / "Más herramientas" (`en:345`/`es:340`) |

Contenido del desplegable «Más herramientas», en orden (`C/Toolbar.tsx:118-217`):

| Entrada | Atajo mostrado | Icono | Etiqueta en / es |
|---|---|---|---|
| Insertar imagen (si `UIOptions.tools.image !== false`) | 9 | `ImageIcon` | "Insert image" / "Insertar imagen" (`en:324`/`es:320`) |
| Marco | F | `frameToolIcon` | "Frame tool" / "Herramienta Estructura" (`en:337`/`es:333`) |
| Web embed | — | `EmbedIcon` | "Web Embed" / "Incrustar Web" (`en:340`/`es:335`) |
| Dibujo a forma | Shift+X (`C/Tools.tsx:139-144`) | `drawShapeToolIcon` | "Draw to shape" / "" (`en:342`/`es:337`) |
| Puntero láser | K | `laserPointerToolIcon` | "Laser pointer" / "Puntero láser" (`en:341`/`es:336`) |
| Bote de pintura | B | `bucketFillIcon` | "Bucket fill" / "" (`en:343`/`es:338`) |
| Lazo (solo en panel «full») | — | `LassoIcon` | "Lasso selection" / "Selección de lazo" (`en:323`/`es:319`) |
| Rótulo «Generate» (texto fijo, 14 px, peso 600, margen 6px 0) (`C/Toolbar.tsx:195-197`) | | | |
| Texto a diagrama (túnel IA, si `aiEnabled`) | | | |
| Mermaid a Excalidraw | — | `mermaidLogoIcon` | "Mermaid to Excalidraw" / "Mermaid a Excalidraw" (`en:346`/`es:341`) |
| Esquema a código (si IA y plugin) | — | `MagicIcon` | "Wireframe to code" / "Esquema a código" (`en:338`/`es:334`) |

El desplegable: `margin-top:.375rem; right:0; min-width:11.875rem` (190 px)
(`C/Toolbar.scss:49-54`).

Tooltip de cada herramienta: `"<Etiqueta> — <letra> or <número>"` (`C/Tools.tsx:183-190`,
`C/Tools.tsx:290`); distintivo de tecla en la esquina = la letra, o el número si no hay
letra (`C/Tools.tsx:291-295`). Pulsar Selección estando ya activa cambia a Lazo
(`C/Tools.tsx:345-359`).

### 1.3 Menú hamburguesa (arriba izquierda)

- Disparador `DropdownMenu.Trigger` con `HamburgerMenuIcon`, clase `main-menu-trigger`
  (`C/main-menu/MainMenu.tsx:43-55`); es un `.dropdown-menu-button`
  (`C/dropdownMenu/DropdownMenuTrigger.tsx:20-21`).
- Contenido alineado `align="start"` (`C/main-menu/MainMenu.tsx:56-63`), dentro de un
  `Island padding={2}` (`C/dropdownMenu/DropdownMenuContent.tsx:107`).

Entradas por defecto, en orden (`C/LayerUI.tsx:111-136`):

| Entrada | Icono | Atajo | Etiqueta en / es |
|---|---|---|---|
| Abrir | `LoadIcon` (`C/main-menu/DefaultItems.tsx:101`) | Ctrl+O (`ACT/shortcuts.ts:61`) | `buttons.load` "Open" / "Abrir" (`en:238`/`es:234`) |
| Guardar en archivo actual (si hay archivo activo) | `save` (`DefaultItems.tsx:126`) | Ctrl+S (`ACT/shortcuts.ts:60`) | `buttons.save` "Save to current file" / "Guardar en archivo actual" (`en:236`/`es:232`) |
| Guardar en… (si `canvasActions.export`) | `ExportIcon` (`DefaultItems.tsx:360`) | — | `buttons.export` "Save to..." / "Guardar en..." (`en:233`/`es:229`) |
| Exportar imagen… (si `saveAsImage`) | `ExportImageIcon` (`DefaultItems.tsx:138`) | Ctrl+Shift+E (`ACT/shortcuts.ts:63`) | `buttons.exportImage` "Export image..." / "Exportar imagen..." (`en:232`/`es:228`) |
| Encontrar en lienzo | `searchIcon` (`DefaultItems.tsx:178`) | Ctrl+F (`ACT/shortcuts.ts:113`) | `search.title` "Find on canvas" / "Encontrar en lienzo" (`en:221`/`es:217`) |
| Ayuda | `HelpIcon` (`DefaultItems.tsx:201`) | ? (`DefaultItems.tsx:203`) | `helpDialog.title` "Help" / "Ayuda" (`en:476`/`es:468`) |
| Reiniciar lienzo | `TrashIcon` (`DefaultItems.tsx:224`) | — | `buttons.clearReset` "Reset the canvas" / "Limpiar lienzo y reiniciar el color de fondo" (`en:230`/`es:226`) |
| — separador — | | | |
| Grupo «Excalidraw links»: GitHub, X, Discord | `GithubIcon`, `XBrandIcon`, `DiscordIcon` (`DefaultItems.tsx:373-399`) | | "Follow us" / "Síguenos" (`en:179`/`es:178`), "Discord chat" / "Chat de Discord" (`en:180`/`es:179`) |
| — separador — | | | |
| Modo oscuro / claro | `MoonIcon` / `SunIcon` (`DefaultItems.tsx:306`) | Shift+Alt+D (`ACT/shortcuts.ts:59`) | "Dark mode" / "Modo oscuro" (`en:254`/`es:250`), "Light mode" / "Modo claro" (`en:255`/`es:251`) |
| Fondo del lienzo (selector de 5 colores) | — | — | `labels.canvasBackground` "Canvas background" / "Fondo del lienzo" (`en:100`/`es:99`) |

Colores rápidos del fondo del lienzo: `#ffffff`, `#f8f9fa`, `#f5faff`, `#fffce8`,
`#fdf8f6` (`COM/colors.ts:284-294`).

### 1.4 Panel de propiedades (izquierda, bajo el menú)

- Solo aparece si `showSelectedShapeActions`: no modo vista, y (herramienta activa
  distinta de selection/lasso/eraser/hand/laser, o editando texto) **o** hay algo
  seleccionado (`ELE/showSelectedShapeActions.ts:7-22`, `C/LayerUI.tsx:302-303`).
- Es un `Island padding={2}` con clase `App-menu__left` (= `CLASSES.SHAPE_ACTIONS_MENU`,
  `COM/constants.ts:106`) y `maxHeight: appState.height - 166 px`
  (`C/LayerUI.tsx:278-288`).
- Contenido detallado en la sección 3.

### 1.5 Abajo a la izquierda: zoom y deshacer/rehacer

- `Stack.Col gap={2}` con `ZoomActions` y `UndoRedoActions` (`C/footer/Footer.tsx:47-63`).
- Zoom: fila `[−] [NN%] [+]` (`C/Actions.tsx:884-896`):
  - `zoomOut`: `ZoomOutIcon`, título "Zoom out — Ctrl+-", deshabilitado en `MIN_ZOOM`
    (`ACT/actionCanvas.tsx:206-210`).
  - `resetZoom`: texto `(zoom*100).toFixed(0)%`, título "Reset zoom"
    (`ACT/actionCanvas.tsx:259-266`).
  - `zoomIn`: `ZoomInIcon`, título "Zoom in — Ctrl++", deshabilitado en `MAX_ZOOM`
    (`ACT/actionCanvas.tsx:159-163`).
  - Etiquetas: "Zoom out"/"Alejarse" (`en:244`/`es:240`), "Reset zoom"/"Restablecer zoom"
    (`en:245`/`es:241`), "Zoom in"/"Acercarse" (`en:243`/`es:239`).
- Deshacer/rehacer: dos botones pegados, con tooltip (`C/Actions.tsx:898-913`);
  `UndoIcon` (Ctrl+Z) y `RedoIcon` (Ctrl+Shift+Z o Ctrl+Y)
  (`ACT/actionHistory.tsx:70-80`, `109-120`). Van a la derecha del zoom con
  `margin-inline-start: 0.6em` (`CSS/styles.scss:639-646`). Etiquetas "Undo"/"Deshacer"
  (`en:249`/`es:245`), "Redo"/"Rehacer" (`en:250`/`es:246`).

### 1.6 Abajo a la derecha: ayuda

`HelpButton` con `HelpIcon`, tooltip "Help — ?"; abre el diálogo de atajos
(`C/HelpButton.tsx:12-23`, `C/footer/Footer.tsx:78-82`).

### 1.7 Arriba a la derecha: biblioteca

`DefaultSidebar.Trigger` con icono **`sidebarRightIcon`** (no `LibraryIcon`) y título
"Library"/"Biblioteca" (`C/LayerUI.tsx:481-499`, `en:332`/`es:328`). Sin texto: la
etiqueta solo aparece si se pasan `children` (`C/Sidebar/SidebarTrigger.tsx:46-48`); en
móvil se oculta (`C/Sidebar/SidebarTrigger.scss:45-47`). Se esconde cuando la barra
lateral está anclada y abierta (`C/LayerUI.tsx:420-426`).

---

## 2. Medidas, variables CSS y tema

### 2.1 Variables (claro → oscuro)

Todas en `.excalidraw` (claro, `CSS/theme.scss:5-177`) y `.excalidraw.theme--dark`
(oscuro, `CSS/theme.scss:185-280`).

| Variable | Claro | Oscuro |
|---|---|---|
| `--space-factor` | 0.25rem = 4 px (`:80`) | igual |
| `--default-button-size` | 2rem = 32 px (`:47`) | igual |
| `--default-icon-size` | 1rem = 16 px (`:48`) | igual |
| `--lg-button-size` | 2.25rem = 36 px (`:49`) | igual |
| `--lg-icon-size` | 1rem = 16 px (`:50`) | igual |
| (pantallas `min-device-width:1921px`) | lg 2.5rem=40 px, lg-icon 1.25rem=20 px, default 2.25rem=36 px, default-icon 20 px (`:172-177`) | igual |
| `--editor-container-padding` | 1rem = 16 px (`:51`) | igual |
| `--mobile-action-button-size` | 2rem (`:52`) | igual |
| `--border-radius-md` | 0.375rem = 6 px (`:149`) | igual |
| `--border-radius-lg` | 0.5rem = 8 px (`:150`) | igual |
| `--island-bg-color` | `#ffffff` (`:25`) | `#232329` (`:205`) |
| `--island-bg-color-alt` | `#fff` (`:26`) | `hsl(240, 12%, 12%)` (`:206`) |
| `--popup-bg-color` | `var(--island-bg-color)` (`:32`) | igual |
| `--popup-secondary-bg-color` | `$color-gray-1` = `#f1f3f5` (`:33`, `CSS/variables.module.scss:7`) | `#222` (`:210`) |
| `--popup-text-color` | `#000` (`:34`) | `$color-gray-4` = `#ced4da` (`:211`) |
| `--select-highlight-color` | `$color-blue-5` = `#339af0` (`:36`) | `$color-blue-4` = `#4dabf7` (`:213`) |
| `--focus-highlight-color` | `$color-blue-2` = `#a5d8ff` (`:17`) | `$color-blue-6` = `#228be6` (`:198`) |
| `--icon-fill-color` | `var(--color-on-surface)` (`:18`) | (hereda: `#e3e3e8`) |
| `--text-primary-color` | `var(--color-on-surface)` (`:81`) | igual |
| `--keybinding-color` | `var(--color-gray-40)` = `#b8b8b8` (`:27`) | `var(--color-gray-60)` = `#7a7a7a` (`:207`) |
| `--button-hover-bg` | `var(--color-surface-high)` (`:42`) | (hereda: `#2e2d39`) |
| `--button-active-bg` | `var(--color-surface-high)` (`:43`) | igual |
| `--button-active-border` | `var(--color-brand-active)` (`:44`) | (hereda: `#d0ccff`) |
| `--default-border-color` | `var(--color-surface-high)` (`:45`) | (hereda) |
| `--button-gray-1/2/3` | `#e9ecef` / `#ced4da` / `#adb5bd` (`:9-11`) | `#363636` / `#272727` / `#222` (`:190-192`) |
| `--input-bg-color` | `#fff` (`:21`) | `#121212` (`:201`) |
| `--input-border-color` | `#ced4da` (`:22`) | `#2e2e2e` (`:202`) |
| `--default-bg-color` | `#fff` (`:20`) | `#121212` (`:200`) |
| `--color-selection` | `#6965db` (`:83`) | `#b4b0ff` (`:229`) |
| `--color-primary` | `#6965db` (`:87`) | `#a8a5ff` (`:233`) |
| `--color-primary-darker` | `#5b57d1` (`:88`) | `#b2aeff` (`:234`) |
| `--color-primary-darkest` | `#4a47b1` (`:89`) | `#beb9ff` (`:235`) |
| `--color-primary-light` | `#e3e2fe` (`:90`) | `#4f4d6f` (`:236`) |
| `--color-primary-light-darker` | `#d7d5ff` (`:91`) | `#43415e` (`:237`) |
| `--color-primary-hover` | `#5753d0` (`:92`) | `#bbb8ff` (`:238`) |
| `--color-surface-high` | `#f1f0ff` (`:152`) | `#2e2d39` (`:268`) |
| `--color-surface-mid` | `#f6f6f9` (`:153`) | `hsl(240 6% 10%)` (`:270`) |
| `--color-surface-low` | `#ececf4` (`:154`) | `hsl(240, 8%, 15%)` (`:269`) |
| `--color-surface-lowest` | `#ffffff` (`:155`) | `hsl(0, 0%, 7%)` (`:271`) |
| `--color-on-surface` | `#1b1b1f` (`:156`) | `#e3e3e8` (`:272`) |
| `--color-brand-hover` | `#5753d0` (`:157`) | `#bbb8ff` (`:273`) |
| `--color-brand-active` | `#4440bf` (`:160`) | `#d0ccff` (`:276`) |
| `--color-on-primary-container` | `#030064` (`:158`) | `#e0dfff` (`:274`) |
| `--color-surface-primary-container` | `#e0dfff` (`:159`) | `#403e6a` (`:275`) |
| `--color-border-outline` | `#767680` (`:161`) | `#8e8d9c` (`:277`) |
| `--color-border-outline-variant` | `#c5c5d0` (`:162`) | `#46464f` (`:278`) |
| `--color-gray-10…100` | `#f5f5f5 #ebebeb #d6d6d6 #b8b8b8 #999999 #7a7a7a #5c5c5c #3d3d3d (85:#242424) #1e1e1e #121212` (`:94-104`) | igual |
| `--color-disabled` | `var(--color-gray-40)` = `#b8b8b8` (`:106`) | `var(--color-gray-70)` = `#5c5c5c` (`:240`) |
| `--color-danger` | `#db6965` (`:114`) | `#ffa8a5` (`:244`) |
| `--color-slider-track` | `hsl(240, 100%, 90%)` (`:57`) | `hsl(244, 23%, 39%)` (`:226`) |
| `--color-slider-thumb` | `var(--color-gray-80)` = `#3d3d3d` (`:58`) | igual |
| `--scrollbar-thumb` | `var(--button-gray-2)` (`:54`) | `$color-gray-8` = `#343a40` (`:223`) |
| `--theme-filter` | `none` (`:6`) | `invert(93%) hue-rotate(180deg)` (`:186`) |

### 2.2 Sombras exactas

- `--shadow-island` (islas: barra, panel, menús):
  `0px 0px 1px 0px rgba(0,0,0,0.17), 0px 0px 3px 0px rgba(0,0,0,0.08), 0px 7px 14px 0px rgba(0,0,0,0.05)`
  (`CSS/theme.scss:37-38`). Aplicada en `C/Island.scss:6`; quitada en modo zen
  (`C/Island.scss:11-13`).
- `--shadow-island-stronger`: igual pero la última capa `rgb(0 0 0 / 18%)`
  (`CSS/theme.scss:39-40`); la usa el selector de puntas de flecha (`C/IconPicker.scss:9`).
- `--modal-shadow` / `--sidebar-shadow`: `0px 100px 80px rgba(0,0,0,0.07), 0px 41.7776px 33.4221px rgba(0,0,0,0.0503198), 0px 22.3363px 17.869px rgba(0,0,0,0.0417275), 0px 12.5216px 10.0172px rgba(0,0,0,0.035), 0px 6.6501px 5.32008px rgba(0,0,0,0.0282725), 0px 2.76726px 2.21381px rgba(0,0,0,0.0196802)`
  (`CSS/theme.scss:60-72`).
- Menú oscuro: además `0 0 0 1px rgba(0,0,0,0.15)` (`C/dropdownMenu/DropdownMenu.scss:70-73`).
- Botones «sobre el lienzo» (hamburguesa, ayuda, biblioteca): `border:none; box-shadow: 0 0 0 1px var(--color-surface-lowest); background: var(--color-surface-low)`; al pulsar `0 0 0 1px var(--color-brand-active)` (`CSS/variables.module.scss:239-246`, aplicado en `CSS/styles.scss:613-615`, `838-840`, `C/Sidebar/SidebarTrigger.scss:5-6`).
- Contenedor de zoom y deshacer: `box-shadow: 0 0 0 1px var(--color-surface-lowest)` (`C/Actions.scss:1-6`).
- Menú contextual: `0 3px 10px rgba(0,0,0,0.2)` (`C/ContextMenu.scss:12`, `color.adjust(#000,$alpha:-0.8)`).
- Paleta emergente de color antigua `.color-picker`: `rgba(0,0,0,.25) 0 1px 4px` (`C/ColorPicker/ColorPicker.scss:226`).

### 2.3 Island

`background: --island-bg-color; box-shadow: --shadow-island; border-radius: --border-radius-lg (8 px); padding: calc(--padding × --space-factor)`, donde `--padding` es la prop `padding` (`C/Island.scss:2-10`, `C/Island.tsx:35`).

### 2.4 Barra de herramientas

- Botón de herramienta: `.ToolIcon__icon` = `--default-button-size` (`C/ToolIcon.scss:46-56`), pero dentro de la barra `.App-toolbar-container .ToolIcon__icon` = **`--lg-button-size` 36 × 36 px**, icono **`--lg-icon-size` 16 px** (`C/ToolIcon.scss:175-193`); radio **8 px** (`C/ToolIcon.scss:5`, `56`).
- Padding de la isla: `padding={1}` → **4 px** (`C/Toolbar.tsx:248`); hueco entre botones **4 px** (`C/Toolbar.tsx:262`).
- **Alto de la barra: 36 + 2·4 = 44 px** (derivado).
- Ancho sin lápiz detectado (derivado): 13 botones × 36 + 2 separadores × 1 + márgenes 4+4 + 15 huecos × 4 + padding 8 = **546 px**.
- Separador: 1 px × 1.5rem (24 px), `--default-border-color` (`C/Toolbar.scss:18-23`).
- Distintivo de tecla: `font-size .625rem` (10 px), `--keybinding-color`, `font-family: --ui-font` (`C/ToolIcon.scss:155-163`); dentro de `.ToolIcon` se coloca a `bottom:4px; right:4px` (`CSS/variables.module.scss:54-57`).
- Botón «más herramientas»: `.dropdown-menu-button` 36 × 36, radio 8 px, borde 1 px (`C/dropdownMenu/DropdownMenu.scss:234-237`, `CSS/variables.module.scss:107-128`), anulado en la barra a `box-shadow:none; border:0; background:transparent` (`C/Toolbar.scss:26-39`).

### 2.5 Menú y botones sobre el lienzo

- Hamburguesa: 36 × 36, `padding .625rem`, radio 8 px, borde 1 px, fondo `--color-surface-mid` y hover `--color-surface-high` (oscuro: fondo `surface-high`, hover `#363541`) (`C/dropdownMenu/DropdownMenu.scss:234-258`) **y** `filledButtonOnCanvas` (`CSS/styles.scss:838-840`). Las dos reglas tienen la misma especificidad (`.excalidraw .x`): cuál gana depende del orden del CSS empaquetado, que no se ve en el fuente.
- Menú desplegado: `max-width: 20rem` (`DropdownMenu.scss:5`); contenedor con hueco 1 px entre entradas (`:64`), alto máx. `100svh - 2·padding - 2.25rem` (`:68`); entrada **2rem (32 px)** de alto (2.25rem en ≥1921 px), `padding 0 .5rem`, radio 6 px, texto **0.875rem (14 px)**, icono 16 px, hueco icono-texto `0.625rem` (`:76-86`, `:134-156`, `:199-202`); atajo a la derecha con `opacity .5` (`:158-161`); título de grupo 14 px peso 500 margen 10px 0 (`:226-231`).
- Ayuda: `.help-icon` 36 × 36, icono 16 px (`CSS/styles.scss:613-624`).
- Biblioteca: `.sidebar-trigger` alto 36 px, ancho automático, hueco .5rem, `font-size .75rem`, `letter-spacing .4px`, icono 16 px (`C/Sidebar/SidebarTrigger.scss:4-23`).
- Zoom/deshacer: botones `--lg-button-size` 36 × 36, `border-radius:0`, fondo `--color-surface-low`, texto 0.875rem, icono 16 px (`C/Actions.scss:8-25`); porcentaje **3.75rem (60 px)** de ancho con `padding 0 .625rem` (`C/Actions.scss:27-34`); esquinas exteriores 8 px (`C/Actions.scss:36-62`, `64-93`).

### 2.6 Panel de propiedades

- `.App-menu__left`: **ancho 12.5rem (200 px)**, `padding: .75rem`, `overflow-y:auto`, `position:absolute` (`CSS/styles.scss:494-500`). El `Island padding={2}` pone además `padding: 8px` en línea con la misma especificidad (`C/Island.scss:8`); igual que arriba, el ganador depende del orden del CSS.
- Dentro del panel: `--button-border: transparent; --button-bg: var(--color-surface-mid)`; oscuro: `--button-hover-bg: #363541; --button-bg: var(--color-surface-high)` (`CSS/styles.scss:842-854`, igual en `C/Actions.scss:200-208`). `.buttonList` con `padding: .25rem 0` (`CSS/styles.scss:856-860`).
- Secciones (`.selected-shape-actions`): columna con **row-gap 0.75rem (12 px)** (`CSS/styles.scss:158-161`).
- Títulos `h3`/`legend`/`.control-label`: `font-size 0.75rem (12 px)`, peso 400, `margin-bottom .25rem`, color `--text-primary-color` (`CSS/styles.scss:163-172`).
- `.buttonList`: `display:flex; flex-wrap:wrap; column-gap .5rem; row-gap .5rem` (8 px) (`CSS/styles.scss:188-192`).
- **Botón de opción** (lo que en Excalidraw antiguo era `ButtonIconSelect`; hoy `RadioSelection` → `<label>` con `<input type=radio>` oculto, o `RadioButton` `<button>` en modo `type="button"`) (`C/RadioSelection.tsx:31-56`, `C/RadioButton.tsx:21-31`). Estilo `outlineButtonIconStyles` (`CSS/styles.scss:254-258`, `CSS/variables.module.scss:107-192`):
  - **32 × 32 px** (`--default-button-size`), `padding:0`, icono **16 px** (`--default-icon-size`).
  - Borde 1 px `--button-border` (transparente en el panel), **radio 8 px**, fondo `--button-bg` (`#f6f6f9` / oscuro `#2e2d39`).
  - Estados: ver sección 5.
- **Muestras de color** (`.color-picker__button`):
  - Fila en el panel: rejilla `1fr 20px 1.625rem`, `padding .25rem 0` (`C/ColorPicker/ColorPicker.scss:66-70`): 5 muestras rápidas repartidas `space-between` (`:83-87`), separador 1 px × 1rem (`C/ButtonSeparator.tsx:1-10`) y la muestra del color activo.
  - Muestra rápida: **1.375rem (22 px)**, radio **4 px** (`ColorPicker.scss:89-103`).
  - Muestra del color activo (abre la paleta): **1.625rem (26 px)**, radio 5 px (`ColorPicker.scss:198-202`).
  - Muestra grande de la paleta: **1.875rem (30 px)**, radio **6 px** (`ColorPicker.scss:178-182`).
  - Colores claros llevan contorno `0 0 0 1px --color-outline` (`#ebebeb` / oscuro `#3d3d3d`) (`ColorPicker.scss:5-8`, `93-95`).
  - Transparente: fondo de tablero PNG 16×16 en base64 (`ColorPicker.scss:184-186`).
  - Letra de atajo en la muestra: 10 px monospace peso 500, `right:4px; bottom:2px` (`ColorPicker.scss:214-221`).
- **Paleta emergente**: `PropertiesPopover` a la derecha (`side="right"`, `align="start"`, `alignOffset=-16`, `sideOffset=20`) dentro de `Island padding={3}` (12 px) (`C/PropertiesPopover.tsx:52-55`, `87`). Rejilla **5 columnas de 30 px, hueco 4 px, padding 8 px** (`ColorPicker.scss:301-306`), secciones separadas 0.75rem (`:294-299`); títulos 0.75rem (`:22-26`); campo hex: alto 32 px, texto 0.875rem (`:334-357`, `:411-443`).
- **Opacidad** (`Range`): pista 4 px de alto radio 2 px, pulgar **16 px** redondo `--color-slider-thumb`, `padding-top 10px; padding-bottom 25px`; valor y «0» en 12 px debajo (`C/Range.scss:4-55`). Parte recorrida `--color-slider-track`, resto `--button-bg` (`C/Range.tsx:46`).
- Puntas de flecha: `.picker` padding .5rem, radio 4 px, sombra `--shadow-island-stronger`; iconos de previsualización 36 × 18 px (`C/IconPicker.scss:5-14`, `50-55`).

### 2.7 Tipografía

- UI: `--ui-font: Assistant, system-ui, BlinkMacSystemFont, -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif` (`CSS/styles.scss:41-42`). En Windows sin Assistant instalada cae a `system-ui`/Segoe UI.
- Botones: `font-size 0.8333rem` (13.33 px) (`CSS/styles.scss:62-65`).
- Tamaños usados: 0.625rem distintivos (`C/ToolIcon.scss:159`), 0.75rem títulos y pistas (`CSS/styles.scss:168`, `C/HintViewer.scss:21`), 0.875rem menú y zoom (`DropdownMenu.scss:80`, `C/Actions.scss:12`), 0.7rem atajo del menú contextual (`C/ContextMenu.scss:64`).
- Texto del lienzo: Excalifont (5) por defecto, tamaño 20 (`COM/constants.ts:216`, `261`).

### 2.8 Cómo se hace el modo oscuro

Dos mecanismos distintos:

1. **La interfaz** se oscurece con **variables**: la clase `theme--dark` en `.excalidraw` redefine los tokens (`CSS/theme.scss:185-280`; clase puesta en `C/App.tsx:4308`). No hay filtro sobre la UI. `--theme-filter` solo se usa en las miniaturas de la biblioteca (`C/LibraryUnit.scss:60`).
2. **El lienzo** transforma **cada color** con `applyDarkModeFilter`: `invert(93%)` y después `hue-rotate(180deg)`, calculado en JS con la matriz de CSS (`COM/colors.ts:16-17`, `19-60`, `62-84`, `86-122`; constante `DARK_THEME_FILTER` en `COM/constants.ts:197`). Se aplica al pintar elementos (`ELE/renderElement.ts:387`, `402`, `444`, `577`, `879`, `932`), a imágenes con `context.filter` (`ELE/renderElement.ts:543-545`), al exportar (`EX/packages/excalidraw/scene/export.ts:466`) y **a las muestras del selector de color** (`C/ColorPicker/TopPicks.tsx:118`, `PickerColorList.tsx:86`, `ShadeList.tsx:64`, `ColorPicker.tsx:286`). El documento guarda siempre el color claro.

---

## 3. Panel de propiedades

### 3.1 Orden y condición de cada sección

Orden del JSX de `SelectedShapeActions` (`C/Actions.tsx:167-223`). Cada condición es un
predicado de `getShapeActionPredicates` (`C/shapeActionPredicates.ts:88-189`). Regla
general: una propiedad aparece si **la herramienta activa la admite o ALGÚN elemento
seleccionado la admite** (`forToolOrSelection`, `C/shapeActionPredicates.ts:98-102`), no
solo si todos la admiten.

Tablas de tipos (`ELE/comparisons.ts`):
- `hasBackground`: rectangle, stickynote, iframe, embeddable, ellipse, diamond, line, freedraw, autoshape, bucketfill (`:3-14`).
- `hasFillStyle`: lo anterior salvo stickynote (`:16-17`).
- `hasStrokeColor`: rectangle, stickynote, ellipse, diamond, freedraw, arrow, line, text, embeddable, autoshape (`:19-29`).
- `hasStrokeWidth`: rectangle, iframe, embeddable, ellipse, diamond, freedraw, arrow, line, autoshape (`:31-40`).
- `hasStrokeStyle`: rectangle, iframe, embeddable, ellipse, diamond, arrow, line, autoshape (`:42-50`).
- `hasRoughness`: `hasStrokeStyle` + stickynote (`:52-53`).
- `canChangeRoundness`: rectangle, iframe, embeddable, line, diamond, stickynote, image (`:57-64`).
- `toolIsArrow` y `canHaveArrowheads`: arrow (`:66-68`).

| # | Sección | Acción | Aparece cuando | Controles (icono) | Etiqueta en / es |
|---|---|---|---|---|---|
| 1 | Trazo | `changeStrokeColor` (`ACT/actionProperties.tsx:358-435`) | `canChangeStrokeColor` (`C/shapeActionPredicates.ts:41-62`) | `h3` + selector de color (5 rápidos + activo) | `labels.stroke` "Stroke" / "Trazo" (`en:29`/`es:29`); en nota adhesiva `labels.textColor` "Text color" / — (`en:30`) (`actionProperties.tsx:407-408`) |
| 2 | Fondo | `changeBackgroundColor` (`:437-551`) | `canChangeBackgroundColor` (`shapeActionPredicates.ts:64-79`) | `h3` + selector | `labels.background` "Background" / "Fondo" (`en:32`/`es:31`) |
| 3 | Relleno | `changeFillStyle` (`:606-689`) | herramienta bucketfill, o `hasFillStyle` con fondo **no transparente** (herramienta o algún elemento) (`shapeActionPredicates.ts:130-139`) | hachure `FillHachureIcon` (o zigzag `FillZigZagIcon`; Alt+clic alterna), cross-hatch `FillCrossHatchIcon`, solid `FillSolidIcon` (`:641-663`, `672-683`) | `labels.fill` "Fill" / "Rellenar" (`en:34`/`es:33`); "Hachure"/"Folleto" (`en:85`/`es:84`), "Zigzag" (`en:86`/`es:85`), "Cross-hatch"/"Rayado transversal" (`en:87`/`es:86`), "Solid"/"Sólido" (`en:84`/`es:83`) |
| 4 | Grosor del trazo | `changeStrokeWidth` (`:708-764`) | `hasStrokeWidth` | **thin** `StrokeWidthBaseIcon`, **medium** `StrokeWidthBoldIcon`, **bold** `StrokeWidthExtraBoldIcon` (`:731-750`) — no hay «extra bold»: valores 1/2/4 px (dibujo libre 0.5/1/2) (`COM/constants.ts:455-493`) | `labels.strokeWidth` "Stroke width" / "Grosor del trazo" (`en:35`/`es:34`); "Thin"/"Fino" (`en:88`/`es:87`), "Medium"/"Mediana" (`en:81`/`es:80`), "Bold"/"Grueso" (`en:89`/`es:88`) |
| 5 | Estilo del trazo | `changeStrokeStyle` (`:901-954`) | `hasStrokeStyle` | solid `StrokeWidthBaseIcon` (sí, el mismo del grosor fino, `:928`), dashed `StrokeStyleDashedIcon`, dotted `StrokeStyleDottedIcon` | `labels.strokeStyle` "Stroke style" / "Estilo del trazo" (`en:36`/`es:35`); "Solid"/"Sólido", "Dashed"/"Discontinua", "Dotted"/"Punteado" (`en:37-39`/`es:36-38`) |
| 5b | Presión (solo dibujo libre) | `changeFreedrawMode` (`:820-899`) | `hasFreedrawMode` (freedraw) | constant `strokeVariabilityConstantIcon`, variable `strokeVariabilityVariableIcon` (`:880-891`) | `labels.pressure` "Pressure" / "" (`en:41`/`es:40`) |
| 6 | Trazo a mano (sloppiness) | `changeSloppiness` (`:766-818`) | `hasRoughness` | architect 0 `SloppinessArchitectIcon`, artist 1 `SloppinessArtistIcon`, cartoonist 2 `SloppinessCartoonistIcon` (`:788-804`) | `labels.sloppiness` "Sloppiness" / **"Estilo de trazo"** (`en:40`/`es:39`, ojo: casi igual que la 5); "Architect"/"Arquitecto", "Artist"/"Artista", "Cartoonist"/"Caricatura" (`en:94-96`/`es:93-95`) |
| 7 | Bordes | `changeRoundness` (`:1744-1800`) | `canChangeRoundness` | sharp `EdgeSharpIcon`, round `EdgeRoundIcon` (`:1790-1799`) | `labels.edges` "Edges" / "Bordes" (`en:46`/`es:45`); "Sharp"/"Afilado", "Round"/"Redondo" (`en:47-48`/`es:46-47`) |
| 8 | Tipo de flecha | `changeArrowType` (`:2053`, panel `:2242-2266`) | `toolIsArrow` | sharp `sharpArrowIcon`, round `roundArrowIcon`, elbow `elbowArrowIcon` | `labels.arrowtypes` "Arrow type" / "Tipo de flecha" (`en:70`/`es:69`); "Sharp arrow"/"Flecha Afilada", "Curved arrow"/"Flecha Curva", "Elbow arrow"/"Flecha de codo" (`en:71-73`/`es:70-72`) |
| 9 | Familia de fuente | `changeFontFamily` (`:1161`, panel `:1435-1470`) | herramienta text o algún texto (`shapeActionPredicates.ts:151`) | 3 botones por defecto: Excalifont `FreedrawIcon` "Hand-drawn", Nunito `FontFamilyNormalIcon` "Normal", Comic Shanns `FontFamilyCodeIcon` "Code"; separador; disparador «más fuentes» (`C/FontPicker/FontPicker.tsx:23-42`, `94-110`) | `labels.fontFamily` "Font family" / "Tipo de fuente" (`en:75`/`es:74`); "Hand-drawn"/"Dibujado a mano", "Normal", "Code"/"Código" (`en:77-79`/`es:76-78`) |
| 10 | Tamaño de fuente | `changeFontSize` (`:999-1090`) | igual que 9 | S 16 `FontSizeSmallIcon`, M 20 `FontSizeMediumIcon`, L 28 `FontSizeLargeIcon`, XL 36 `FontSizeExtraLargeIcon` (`:1025-1048`, `COM/constants.ts:115-120`) | `labels.fontSize` "Font size" / "Tamaño de la fuente" (`en:74`/`es:73`); "Small"/"Pequeña", "Medium"/"Mediana", "Large"/"Grande", "Very large"/"Muy grande" (`en:80-83`/`es:79-82`) |
| 11 | Alineación de texto | `changeTextAlign` (`:1543-1640`) | herramienta text o `suppportsHorizontalAlign` (`shapeActionPredicates.ts:152-154`) | left `TextAlignLeftIcon`, center `TextAlignCenterIcon`, right `TextAlignRightIcon` (`:1586-1603`) | `labels.textAlign` "Text align" / "Alineado de texto" (`en:45`/`es:44`); "Left"/"Izquierda", "Center"/"Centrado", "Right"/"Derecha" (`en:90-92`/`es:89-91`) |
| 11b | Alineación vertical | `changeVerticalAlign` (`:1644-1701`) | `shouldAllowVerticalAlign` (texto dentro de contenedor) (`shapeActionPredicates.ts:155`) | top/middle/bottom `TextAlignTopIcon`/`TextAlignMiddleIcon`/`TextAlignBottomIcon` (variantes por tema) | "Align top"/"Alineación superior", "Center vertically"/"Centrar verticalmente", "Align bottom"/"Alineación inferior" (`en:124-125,128`/`es:123-124,127`) |
| 12 | Puntas de flecha | `changeArrowhead` (`:1943-2031`) | `canHaveArrowheads` | dos `IconPicker` (inicio, fin) con: None, Arrow, Triangle, Triangle outline (visibles); Circle, Circle outline, Diamond, Diamond outline, Bar; grupo Cardinality (6) (`:1829-1930`) | `labels.arrowheads` "Arrowheads" / "Puntas de flecha" (`en:49`/`es:48`); "None"/"Ninguna" … (`en:50-58`/`es:49-57`), "Cardinality"/"Cardinalidad" (`en:69`/`es:68`) |
| 13 | Opacidad | `changeOpacity` (`:956-997`) | siempre salvo herramienta autoshape sin selección (`shapeActionPredicates.ts:157`) | `Range` **0–100, paso 10** (`:985-994`) | `labels.opacity` "Opacity" / "Opacidad" (`en:44`/`es:43`) |
| 14 | Capas | `LayersFieldset` (`C/Actions.tsx:65-79`) | salvo freedraw/autoshape activos sin selección (`shapeActionPredicates.ts:162-168`) | enviar al fondo `SendToBackIcon`, enviar atrás `SendBackwardIcon`, traer adelante `BringForwardIcon`, traer al frente `BringToFrontIcon` | `labels.layers` "Layers" / "Capas" (`en:103`/`es:102`); "Send to back"/"Enviar al fondo" (`en:23`/`es:23`), "Send backward"/"Enviar atrás" (`en:25`/`es:25`), "Bring forward"/"Traer hacia delante" (`en:22`/`es:22`), "Bring to front"/"Traer al frente" (`en:24`/`es:24`) |
| 15 | Alinear | `AlignFieldset` (`C/Actions.tsx:85-130`) | **2+ grupos/elementos seleccionados** y ningún marco (`ACT/actionAlign.tsx:39-53`), y no es un único contenedor con su texto (`shapeActionPredicates.ts:169-170`) | fila 1: izquierda `AlignLeftIcon`, centro horizontal `CenterHorizontallyIcon`, derecha `AlignRightIcon` [+ distribuir horizontal]; fila 2: arriba `AlignTopIcon`, centro vertical `CenterVerticallyIcon`, abajo `AlignBottomIcon` [+ distribuir vertical] | `labels.align` "Align" / "Alinear" (`en:123`/`es:122`); "Align left"/"Alinear a la izquierda" (`en:126`/`es:125`), "Center horizontally"/"Centrar horizontalmente" (`en:129`/`es:128`), "Align right"/"Alinear a la derecha" (`en:127`/`es:126`), "Align top"/"Alineación superior" (`en:124`/`es:123`), "Center vertically" (`en:128`/`es:127`), "Align bottom"/"Alineación inferior" (`en:125`/`es:124`) |
| 15b | Distribuir (dentro de Alinear) | `distributeHorizontally` / `distributeVertically` | **3+ elementos** (`targetElements.length > 2`, `shapeActionPredicates.ts:171`) | `DistributeHorizontallyIcon` (Alt+H), `DistributeVerticallyIcon` (Alt+V) (`ACT/actionDistribute.tsx:93-96`, `124-126`) | "Distribute horizontally"/"Distribuir horizontalmente", "Distribute vertically"/"Distribuir verticalmente" (`en:130-131`/`es:129-130`) |
| 16 | Acciones | `C/Actions.tsx:208-221` | hay selección y no se edita texto ni se crea elemento (`shapeActionPredicates.ts:121`) | duplicar `DuplicateIcon` (Ctrl+D), borrar `TrashIcon`, agrupar `GroupIcon` (Ctrl+G), desagrupar `UngroupIcon` (Ctrl+Shift+G), enlace `LinkIcon` (Ctrl+K; con 1 elemento o contenedor+texto), recortar `cropIcon` (1 imagen), editar línea `lineEditorIcon` (1 línea no-codo) (`shapeActionPredicates.ts:177-187`) | `labels.actions` "Actions" / "Acciones" (`en:104`/`es:103`); "Duplicate"/"Duplicar" (`en:107`/`es:106`), "Delete"/"Borrar" (`en:26`/`es:26`), "Group selection"/"Agrupar selección" (`en:112`/`es:111`), "Ungroup selection"/"No agrupar selección" (`en:113`/`es:112`), "Link"/"Enlace" (`en:152`/`es:151`), "Crop image"/"Recortar imagen" (`en:483`/`es:475`), "Edit line"/"Editar línea" (`en:159`/`es:158`) |

Notas:
- Con la herramienta **bote de pintura** el panel se reduce a fondo (sin transparente),
  relleno y opacidad (`C/Actions.tsx:155-165`, `ACT/actionProperties.tsx:553-604`).
- Los tooltips de alinear llevan su atajo: Ctrl+Shift+↑/↓/←/→ (`ACT/actionAlign.tsx:104-106`, `138-140`, `172-174`, `206-208`); capas: Ctrl+[ / Ctrl+] / Ctrl+Shift+[ / Ctrl+Shift+] (`ACT/actionZindex.tsx:46`, `76`, `109-112`, `147-150`).
- Solo los botones de Alinear invierten su orden en RTL (`C/Actions.tsx:92-110`).

---

## 4. Paleta

### 4.1 Colores base (`COM/colors.ts:193-212`)

`transparent`, black `#1e1e1e`, white `#ffffff`, y cinco tonos (índices 0-4 = pesos
open-color 50/200/400/600/800; bronze = radix 3/5/7/9/11):

| Color | 0 | 1 | 2 | 3 | 4 |
|---|---|---|---|---|---|
| gray | `#f8f9fa` | `#e9ecef` | `#ced4da` | `#868e96` | `#343a40` |
| red | `#fff5f5` | `#ffc9c9` | `#ff8787` | `#fa5252` | `#e03131` |
| pink | `#fff0f6` | `#fcc2d7` | `#f783ac` | `#e64980` | `#c2255c` |
| grape | `#f8f0fc` | `#eebefa` | `#da77f2` | `#be4bdb` | `#9c36b5` |
| violet | `#f3f0ff` | `#d0bfff` | `#9775fa` | `#7950f2` | `#6741d9` |
| blue | `#e7f5ff` | `#a5d8ff` | `#4dabf7` | `#228be6` | `#1971c2` |
| cyan | `#e3fafc` | `#99e9f2` | `#3bc9db` | `#15aabf` | `#0c8599` |
| teal | `#e6fcf5` | `#96f2d7` | `#38d9a9` | `#12b886` | `#099268` |
| green | `#ebfbee` | `#b2f2bb` | `#69db7c` | `#40c057` | `#2f9e44` |
| yellow | `#fff9db` | `#ffec99` | `#ffd43b` | `#fab005` | `#f08c00` |
| orange | `#fff4e6` | `#ffd8a8` | `#ffa94d` | `#fd7e14` | `#e8590c` |
| bronze | `#f8f1ee` | `#eaddd7` | `#d2bab0` | `#a18072` | `#846358` |

### 4.2 Colores rápidos por defecto (5 huecos, `COM/colors.ts:236`)

- **Trazo**: `#1e1e1e`, `#e03131`, `#2f9e44`, `#1971c2`, `#f08c00` — black + red/green/blue/yellow en índice 4 (`COM/colors.ts:190`, `239-245`).
- **Fondo**: `transparent`, `#ffc9c9`, `#b2f2bb`, `#a5d8ff`, `#ffec99` — índice 1 (`COM/colors.ts:191`, `248-254`).
- Bote de pintura: como fondo pero `#ffffff` en vez de transparente (`COM/colors.ts:259-265`).
- Nota adhesiva: fondo `#ffdf6b`, `#fcc2d7`, `#b2f2bb`, `#a5d8ff`, `#ffd8a8`; trazo = el de siempre (`COM/colors.ts:268-281`).
- Los rápidos son personalizables (arrastrar y soltar, menú «reset»): `customizableTopPicks` (`ACT/colorTargets.ts:59`, `C/ColorPicker/TopPicks.tsx:103-178`).

### 4.3 Paleta completa del selector (rejilla 5 × 3)

Orden de `DEFAULT_ELEMENT_STROKE_COLOR_PALETTE` y `..._BACKGROUND_...` (idénticos)
(`COM/colors.ts:299-319`, con `COMMON_ELEMENT_SHADES` en `:217-228`):

| | q/a/z | w/s/x | e/d/c | r/f/v | t/g/b |
|---|---|---|---|---|---|
| fila 1 | transparent | white | gray | black | bronze |
| fila 2 | cyan | blue | violet | grape | pink |
| fila 3 | green | teal | yellow | orange | red |

- Atajos de cada casilla: `q w e r t / a s d f g / z x c v b` (`C/ColorPicker/colorPickerUtils.ts:38-42`, usados en `PickerColorList.tsx:85`).
- Cada casilla con tonos muestra el tono activo: el del color actual o, si no, índice **4 para trazo** e **1 para fondo** (`C/ColorPicker/Picker.tsx:117-122`, `PickerColorList.tsx:69-70`).
- Debajo, la fila «Shades» con los 5 tonos del color elegido, atajos 1-5 (`C/ColorPicker/ShadeList.tsx:97`), y el campo «Hex code» (`C/ColorPicker/ColorPicker.tsx:108`). Encima, «Most used custom colors» si los hay (`C/ColorPicker/Picker.tsx:180-194`). Rótulos: "Colors"/"Colores", "Shades"/"Sombras", "Hex code"/"Código Hexadecimal", "Most used custom colors"/"Colores personalizados más utilizados" (`en:619-622`/`es:611-614`).
- Ocultar un color (transparente en bote/nota) deja la casilla invisible para no mover los atajos (`PickerColorList.tsx:72-83`).

### 4.4 Valores por defecto de un elemento nuevo

| Propiedad | Valor | Origen |
|---|---|---|
| strokeColor | `#1e1e1e` | `COM/constants.ts:507`, `EX/packages/excalidraw/appState.ts:41` |
| backgroundColor | `transparent` | `COM/constants.ts:508`, `appState.ts:32` |
| fillStyle | `solid` | `COM/constants.ts:509`, `appState.ts:34` |
| strokeWidth | medium = 2 | `COM/constants.ts:495`, `510`, `appState.ts:47` |
| strokeStyle | `solid` | `COM/constants.ts:511`, `appState.ts:46` |
| roughness | artist = 1 | `COM/constants.ts:512`, `appState.ts:38` |
| opacity | 100 | `COM/constants.ts:513`, `appState.ts:37` |
| roundness | `round` (`sharp` en tests) | `appState.ts:44` |
| arrowType | `round` | `appState.ts:45` |
| puntas | inicio `null`, fin `arrow` | `appState.ts:33`, `40` |
| fuente | Excalifont (5), 20, `left` | `COM/constants.ts:216`, `261-262`; `appState.ts:35-36`, `48` |
| variabilidad (freedraw) | `constant` | `appState.ts:39` |
| nota adhesiva | trazo `#1e1e1e`, fondo `#ffdf6b` | `appState.ts:42-43`, `COM/colors.ts:268` |
| fondo del lienzo | `#ffffff` | `appState.ts:116` |
| tema | claro | `appState.ts:30` |

---

## 5. Estados de los controles

### 5.1 Botón de herramienta (barra) — mixin `toolbarButtonColorStates`

(`CSS/variables.module.scss:34-105`, incluido en `C/ToolIcon.scss:17`)

| Estado | Regla | Claro | Oscuro |
|---|---|---|---|
| Reposo | sin fondo; icono `--icon-fill-color` (`C/ToolIcon.scss:50`, `66`) | icono `#1b1b1f` | icono `#e3e3e8` |
| Hover | `background: --button-hover-bg` (`:60-62`) | `#f1f0ff` | `#2e2d39` |
| Pulsado (:active) | fondo hover + `border: 1px solid --button-active-border`; icono `--color-on-primary-container` (`:64-71`) | borde `#4440bf`, icono `#030064` | borde `#d0ccff`, icono `#e0dfff` |
| **Seleccionado** (`ToolIcon--checked`) | `background: --color-surface-primary-container`; icono y distintivo `--color-on-primary-container` (`:45-52`) | fondo **`#e0dfff`**, icono `#030064` | fondo **`#403e6a`**, icono `#e0dfff` |
| Seleccionado + `fillable` | además `svg { fill: --icon-fill-color }` = on-primary-container: el icono se **rellena** (`:35-43`; `fillable` en `C/Tools.tsx:79,85,91,97,103,109`) | relleno `#030064` | relleno `#e0dfff` |
| Foco teclado | `box-shadow: 0 0 0 2px --focus-highlight-color` (`C/ToolIcon.scss:41-43`) | `#a5d8ff` | `#228be6` |
| Deshabilitado | sin fondo/borde; icono y distintivo `--color-disabled` (`:73-80`, `:85-104`) | `#b8b8b8` | `#5c5c5c` |
| «Más herramientas» con herramienta extra activa | `background: --color-surface-primary-container; color: --color-on-primary-container` (`C/Toolbar.scss:41-46`) | `#e0dfff` / `#030064` | `#403e6a` / `#e0dfff` |

### 5.2 Botón de opción del panel — mixin `outlineButtonStyles`

(`CSS/variables.module.scss:107-173`; en el panel `--button-bg`/`--button-border` de `CSS/styles.scss:842-854`)

| Estado | Regla | Claro | Oscuro |
|---|---|---|---|
| Reposo | fondo `--button-bg`, borde 1 px `--button-border`, texto/icono `--color-on-surface` (`:117-121`) | fondo `#f6f6f9`, borde transparente | fondo `#2e2d39` |
| Hover | `background --button-hover-bg` (`:129-139`) | `#f1f0ff` | `#363541` |
| Pulsado | `background --button-active-bg`; `border-color --button-active-border` (`:141-144`) | `#f1f0ff` / `#4440bf` | `#2e2d39` / `#d0ccff` |
| **Activo** (`.active`) | fondo y borde `--color-surface-primary-container`; icono `--color-on-primary-container` (`:146-167`) | **`#e0dfff`** / icono `#030064` | **`#403e6a`** / icono `#e0dfff` |
| Foco teclado | `box-shadow: 0 0 0 1px --color-brand-hover` (`CSS/styles.scss:223-230`) | `#5753d0` | `#bbb8ff` |

### 5.3 Otros

- Muestra de color activa: contorno `0 0 0 1px --color-primary-darkest` (`#4a47b1` / `#beb9ff`) (`C/ColorPicker/ColorPicker.scss:143-155`); hover de muestra grande `inset 0 0 0 1px --color-primary`; pulsada `0 0 0 1px --color-primary` (`:109-114`); foco: marco de 3 px `--focus-highlight-color` a 4 px de distancia (`:157-176`).
- Entrada de menú: hover `--button-hover-bg`; pulsada + `0 0 0 1px --color-brand-active`; seleccionada `--color-primary-light` (`#e3e2fe` / `#4f4d6f`) con icono `--color-primary-darker`; deshabilitada `opacity .5` (`C/dropdownMenu/DropdownMenu.scss:169-197`).
- Botones de zoom/deshacer deshabilitados (`ToolIcon_type_button:disabled`): icono `--color-disabled`, sin hover (`C/ToolIcon.scss:105-119`).
- Menú contextual: hover `background --select-highlight-color` (`#339af0` / `#4dabf7`) con texto `--popup-bg-color`; «Borrar» en rojo `$color-red-7` `#f03e3e` y en hover fondo `$color-red-6` `#fa5252` (`C/ContextMenu.scss:50-78`, `CSS/variables.module.scss:3-4`).

---

## 6. Menú contextual

Se abre con `type = "element"` si el clic cae en un elemento o en la caja común de la
selección, si no `"canvas"` (`C/App.tsx:13480`). Cada acción se filtra por su
`predicate`; los separadores duplicados o iniciales se omiten (`C/ContextMenu.tsx:38-53`,
`72-79`). Atajo mostrado a la derecha desde `shortcutMap` (`C/ContextMenu.tsx:119-123`,
`ACT/shortcuts.ts:58-116`).

### 6.1 Lienzo (`C/App.tsx:13910-13927`)

| Entrada | Atajo | en / es |
|---|---|---|
| Pegar | Ctrl+V | "Paste" / "Pegar" (`en:5`/`es:5`) |
| — | | |
| Copiar como PNG | Shift+Alt+C | "Copy to clipboard as PNG" / "Copiar al portapapeles como PNG" (`en:17`/`es:17`) |
| Copiar como SVG | — | "Copy to clipboard as SVG" / "Copiar al portapapeles como SVG" (`en:18`/`es:18`) |
| Copiar como texto | — | "Copy to clipboard as text" / "Copiar al portapapeles como texto" (`en:19`/`es:19`) |
| — | | |
| Seleccionar todo | Ctrl+A | "Select all" / "Seleccionar todo" (`en:12`/`es:12`) |
| Desbloquear todo | — | "Unlock all" / "Desbloquear todo" (`en:170`/`es:169`) |
| — | | |
| Alternar rejilla | Ctrl+' | "Toggle grid" / "Alternar rejilla" (`en:116`/`es:115`) |
| Ajustar a objetos | Alt+S | "Snap to objects" / "Ajustar a los objetos" (`en:258`/`es:254`) |
| Vinculación de flecha | — | "Arrow binding" / "Vinculación de flecha" (`en:198`/`es:197`) |
| Adherirse a puntos medios | — | "Snap to midpoints" / "Adherirse a los puntos medios" (`en:199`/`es:198`) |
| Modo zen | Alt+Z | "Zen mode" / "Modo Zen" (`en:257`/`es:253`) |
| Modo vista | Alt+R | "View mode" / "Modo presentación" (`en:134`/`es:133`) |
| Estadísticas | Alt+/ | "Canvas & Shape properties" / "Propiedades del Lienzo y Forma" (`en:560`/`es:552`) |

En modo vista: copiar PNG, copiar SVG, rejilla, zen, vista, estadísticas (`C/App.tsx:13900-13907`).

### 6.2 Elemento (`C/App.tsx:13933-13989`)

Orden (entre corchetes el atajo):
Cortar [Ctrl+X] · Copiar [Ctrl+C] · Pegar [Ctrl+V] — Seleccionar todo en el marco ·
Quitar todo del marco · Envolver selección en marco — Recortar imagen — Copiar como PNG
[Shift+Alt+C] · Copiar como SVG · Copiar como texto — Copiar estilos [Ctrl+Alt+C] · Pegar
estilos [Ctrl+Alt+V] — Agrupar [Ctrl+G] · Autoajustar texto · Desvincular texto ·
Vincular texto al contenedor · Envolver texto en contenedor · Desagrupar [Ctrl+Shift+G] —
Añadir a la biblioteca — (solo escritorio) Enviar atrás [Ctrl+[] · Traer adelante [Ctrl+]]
· Enviar al fondo [Ctrl+Shift+[] · Traer al frente [Ctrl+Shift+]] — Voltear horizontal
[Shift+H] · Voltear vertical [Shift+V] — Editar línea — Enlace [Ctrl+K] · Copiar enlace al
objeto — Duplicar [Ctrl+D] · Bloquear/Desbloquear [Ctrl+Shift+L] — Borrar [Supr]
(en rojo, `C/ContextMenu.tsx:114`).

Etiquetas: "Cut"/"Cortar" (`en:15`/`es:15`), "Copy"/"Copiar" (`en:16`/`es:16`), "Select all
elements in frame"/"Seleccionar todos los elementos en el marco" (`en:174`/`es:173`),
"Remove all elements from frame"/"Eliminar todos los elementos del marco" (`en:175`/`es:174`),
"Wrap selection in frame"/"Ajustar la selección en el marco" (`en:190`/`es:189`), "Copy
styles"/"Copiar estilos" (`en:27`/`es:27`), "Paste styles"/"Pegar estilos" (`en:28`/`es:28`),
"Enable text auto-resizing"/"Activar redimensionado automático de texto" (`en:185`/`es:184`),
"Unbind text"/"Desvincular texto" (`en:145`/`es:144`), "Bind text to the container"/"Vincular
texto al contenedor" (`en:146`/`es:145`), "Wrap text in a container"/"Envolver el texto en un
contenedor" (`en:147`/`es:146`), "Add to library"/"Añadir a la biblioteca" (`en:117`/`es:116`),
"Flip horizontal"/"Girar horizontalmente" (`en:132`/`es:131`), "Flip vertical"/"Girar
verticalmente" (`en:133`/`es:132`), "Copy link to object"/"Copiar enlace al objeto"
(`en:188`/`es:187`), "Lock"/"Bloquear" y "Unlock"/"Desbloquear" (`en:167-168`/`es:166-167`).
Origen de cada `label`: `ACT/actionClipboard.tsx:24-256`, `ACT/actionFrame.ts:37-127`,
`ACT/actionStyles.ts:52-84`, `ACT/actionTextAutoResize.ts:23-24`,
`ACT/actionBoundText.tsx:61-260`, `ACT/actionAddToLibrary.ts:64`, `ACT/actionFlip.ts:30-56`,
`ACT/actionElementLink.ts:17-18`, `ACT/actionElementLock.ts:26-35`, `218`,
`ACT/actionCropEditor.tsx:14-15`.

### 6.3 Medidas del menú contextual

`border-radius 4px`, `padding .5rem 0`, fondo `--popup-secondary-bg-color`, borde 1 px
`--button-gray-3` (`C/ContextMenu.scss:9-20`); entrada `min-width 9.5rem`,
`padding .25rem 1rem .25rem 1.25rem`, rejilla `1fr 0.2fr` (`:26-41`); marca ✓ a 6 px
(`:43-48`); atajo `opacity .6`, 0.7rem (`:60-65`); separador `border-top 1px $color-gray-5`
= `#adb5bd` (`:98-101`).

---

## 7. Atajos de teclado (diálogo de ayuda)

`C/HelpDialog.tsx`. Rótulos de las islas: "Tools"/"Herramientas" (`en:472`/`es:464`),
"View"/"Vista" (`en:477`/`es:469`), "Editor" (`en:465`/`es:457`).

**Herramientas** (`C/HelpDialog.tsx:145-259`): Mano H · Selección V/1 · Rectángulo R/2 ·
Rombo D/3 · Elipse O/4 · Flecha A/5 · Línea L/6 · Dibujo P/7 · Texto T/8 · Nota N ·
Imagen 9 · Borrador E/0 · Marco F · Láser K · Bote B · Cuentagotas I, Shift+S, Shift+G ·
Editar puntos de línea/flecha Ctrl+Enter · Editar texto Enter · Nueva línea en texto
Enter / Shift+Enter · Terminar texto Esc / Ctrl+Enter · Flecha curva A clic clic clic ·
Línea curva L clic clic clic · Recortar imagen doble clic / Enter · Terminar recorte
Enter / Esc · Bloquear herramienta Q · Evitar enlace de flecha Ctrl · Enlace Ctrl+K ·
Cambiar tipo de forma Tab / Shift+Tab. (Dibujo a forma: Shift+X, `C/Tools.tsx:139-144`.)

**Vista** (`C/HelpDialog.tsx:262-335`): Acercar Ctrl++ · Alejar Ctrl+- · Zoom 100 %
Ctrl+0 · Ajustar a todo Shift+1 · Ampliar selección Shift+2 · Página arriba/abajo
PgUp/PgDn · Página izq./der. Shift+PgUp/PgDn · Zen Alt+Z · Ajustar a objetos Alt+S ·
Rejilla Ctrl+' · Modo vista Alt+R · Tema claro/oscuro Alt+Shift+D · Estadísticas Alt+/ ·
Buscar Ctrl+F · Paleta de comandos Ctrl+/ o Ctrl+Shift+P.
(Discrepancia en el propio Excalidraw: el diálogo dice «Zoom to selection» Shift+2, pero
`shortcuts.ts` asigna Shift+2 a `zoomToFitSelectionInViewport` y Shift+3 a
`zoomToFitSelection`, `ACT/shortcuts.ts:107-109`.)

**Editor** (`C/HelpDialog.tsx:336-516`): Crear diagrama de flujo Ctrl+flecha · Navegarlo
Alt+flecha · Mover lienzo Espacio+arrastrar / rueda+arrastrar · Reiniciar lienzo
Ctrl+Supr · Borrar Supr · Cortar Ctrl+X · Copiar Ctrl+C · Pegar Ctrl+V · Pegar como texto
Ctrl+Shift+V · Seleccionar todo Ctrl+A · Añadir a la selección Shift+clic · Selección
profunda Ctrl+clic · Selección profunda en caja Ctrl+arrastrar · Copiar como PNG
Shift+Alt+C · Copiar estilos Ctrl+Alt+C · Pegar estilos Ctrl+Alt+V · Enviar al fondo
Ctrl+Shift+[ · Traer al frente Ctrl+Shift+] · Enviar atrás Ctrl+[ · Traer adelante Ctrl+]
· Alinear arriba/abajo/izq./der. Ctrl+Shift+↑/↓/←/→ · Duplicar Ctrl+D / Alt+arrastrar ·
Bloquear selección Ctrl+Shift+L · Deshacer Ctrl+Z · Rehacer Ctrl+Y / Ctrl+Shift+Z (fuera
de macOS) · Agrupar Ctrl+G · Desagrupar Ctrl+Shift+G · Voltear H Shift+H · Voltear V
Shift+V · Selector de trazo S · Selector de fondo G · Selector de fuente Shift+F · Menor
fuente Ctrl+Shift+< · Mayor fuente Ctrl+Shift+>.

---

## 8. Plan para Direct2D

### 8.1 Qué hay en `iconos.json` (207 entradas)

Recuento de letras de orden en todos los `d` (medido con un script sobre el JSON):

| Orden | M | m | L | l | H | h | V | v | C | c | S | s | A | a | Z | z |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Veces | 681 | 120 | 123 | 299 | 145 | 253 | 114 | 299 | 78 | 334 | 4 | 26 | 52 | 381 | 127 | 88 |

- **No aparecen Q/q ni T/t.** Arcos elípticos: 433 (A+a).
- `origen` de trazos convertidos a `d` desde otras formas: `line` 44, `circle` 11,
  `polyline` 6, `rect` 15 (el `circle` se escribe como dos arcos `A`).
- `viewBox`: 24×24 (112), 20×20 (48), 40×20 (16, previsualización de puntas), 512×512 (11),
  448×512 (4), 22×22 (2), 182×182 (2), 384×512 (2) y otros sueltos; 3 entradas sin
  `viewBox` son datos, no SVG (`noSvg`: `bucketFillIconSvgPaths`, `eyeDropperIconSvgPaths`,
  `emptyIcon`).
- Propiedades base (`C/icons.tsx:53-75`): `tablerIconProps` (24×24, trazo 2, `round`/`round`,
  107 iconos), `modifiedTablerIconProps` (20×20, sin grosor propio, 47),
  `arrowheadPreviewIconProps` (40×20, 15), sin props 35.
- Valores de pintura: `fill` ∈ {`none`, `#fff`, `currentColor`, `var(--icon-fill-color)`,
  `black`}; `stroke` ∈ {`currentColor`, `none`, `var(--icon-fill-color)`}; `strokeWidth` ∈
  {0, 1, 1.25, 1.5, 1.75, 2, 2.5, 2.75, 3.25, 3.75, 6, 40}; `linecap` ∈ {`round`, `butt`};
  `linejoin` ∈ {`round`, `miter`}.
- `opacity 0.3`: solo `ArrowheadNoneIcon`. `fillRule evenodd`: las tres flechas de la
  pantalla de bienvenida.
- **Transformaciones**: `translate(40, 0) scale(-1, 1)` (puntas de cardinalidad);
  `css:rotate(180deg)` (estilo del `<svg>` de `SendBackwardIcon` y `SendToBackIcon`,
  `C/icons.tsx:782-796`); `rotate(90 10 10)` (`laserPointerToolIcon`). `claseSvg:
  rtl-mirror` (volteo solo en RTL) en `GroupIcon`, `UngroupIcon`, `clone`, `questionCircle`.
- **Máscaras/recortes**: `url(#a)` en 12 iconos (alinear/distribuir) = `clipPath` con un
  rectángulo `M0 0h20v20H0z`, es decir **la caja entera: no recorta nada**;
  `url(#FillHachureIcon)`, `url(#FillCrossHatchIcon)`, `url(#UnlockedIcon)` son `<mask>` con
  `maskType: "alpha"` (`C/icons.tsx:176-190`, `1072-1088`). La de `FillHachureIcon` es un
  rectángulo redondeado **con relleno y trazo de 1.25** (`C/icons.tsx:1081-1086`).
- **Variantes**: 15 puntas de flecha tienen versión `flip:false/true`; `GroupIcon`,
  `UngroupIcon`, `StrokeStyleSolidIcon`, `TextAlignTop/Middle/BottomIcon` tienen versión
  `theme: light/dark` (en los de grupo cambia el color de los tiradores, `#fff` / `#1e1e1e`,
  `C/icons.tsx:18-19`).

### 8.2 Qué hace falta en `pixpin-render`

Estado en el árbol de trabajo en el momento de esta investigación (ficheros **sin commit**
y en curso: `crates/pixpin-render/src/trayecto_svg.rs`, `icono.rs`, `iconos_excalidraw.rs`,
`herramientas/iconos-excalidraw.mjs`; `git status`):

| Necesidad | Estado | Dónde |
|---|---|---|
| Parser de `d` (abs/rel, números pegados `.5.5`, exponentes, banderas pegadas, repetición implícita, M→L) | **Hecho** | `crates/pixpin-render/src/trayecto_svg.rs:35-244` |
| H/V → líneas, S/T con control reflejado, Q/T → cúbicas | **Hecho** | `trayecto_svg.rs:185-222`, `246-266` |
| Arcos elípticos A/a (SVG 1.1 F.6.5-F.6.6, radios agrandados, cúbicas ≤ 90°, cierre exacto) | **Hecho** | `trayecto_svg.rs:268-356` |
| Prueba que lee todos los `d` de `iconos.json` | **Hecho** | `trayecto_svg.rs:477-497` |
| Tramos → `ID2D1PathGeometry1` (figuras abiertas/cerradas, winding/alternate) con caché | **Hecho** | `crates/pixpin-render/src/icono.rs:188-261` |
| Estilo de trazo `round`/`butt` × `round`/`miter` (miterLimit 4) cacheado | **Hecho** | `icono.rs:263-296` |
| `viewBox` → caja sin deformar (preserveAspectRatio) y grosor que escala | **Hecho** | `icono.rs:76-83`, `170-176` |
| `currentColor` | **Hecho** como `Pintura::Actual` | `icono.rs:26-34`, `155-165` |
| `transform` → matriz | **Hecho en el generador** (incluye `css:rotate` alrededor del centro) | `herramientas/iconos-excalidraw.mjs:39-92`; `icono.rs:109-122` |
| Máscara alfa como capa con máscara geométrica | **Hecho**; recorte de caja entera omitido | `icono.rs:123-153`, `iconos-excalidraw.mjs:133-139` |
| `var(--icon-fill-color)` | Resuelto como `Actual` | `iconos-excalidraw.mjs:26-32`. Correcto solo si quien pinta pasa el color del token (on-surface, u on-primary-container en seleccionado). |
| `fill="black"` (logotipos) | **Pendiente**: el generador lo trata como `Actual` (`iconos-excalidraw.mjs:29-31`); en tema oscuro saldría claro en vez de negro. |
| Variantes `flip` y `theme` | **Pendiente**: el generador solo emite `trazos` (una variante por icono, `iconos-excalidraw.mjs:116-131`). Falta la punta de flecha volteada para el selector de inicio y los tiradores oscuros de agrupar/desagrupar. |
| Máscara con trazo (`FillHachureIcon`) | **Pendiente/aproximado**: una máscara geométrica solo cubre el relleno; faltan 0.625 px de borde. Se arregla ensanchando la geometría (`ID2D1Geometry::Widen`) y combinándola. |
| `opacity` de un `<path>` con relleno y trazo | Aproximado: se multiplica el alfa de cada pincel (`icono.rs:155-165`); en SVG la opacidad se aplica al conjunto. Solo afecta a `ArrowheadNoneIcon` (sin relleno visible: la diferencia es nula en la práctica). |
| `rtl-mirror` | No aplica (PixPin no tiene RTL). |
| **Sombras de isla** (`--shadow-island`, 3 capas) | **Falta**: `pixpin-render` no tiene primitiva de sombra (en `lienzo.rs` solo aparece la palabra en un comentario, `:242`). Opciones: efecto `CLSID_D2D1Shadow` sobre la forma, o 9-slice precalculado. |
| Tokens de tema claro/oscuro | **Falta** una tabla Rust con la sección 2.1. |
| `applyDarkModeFilter` para colores del lienzo y muestras | **Falta**: es pura (`COM/colors.ts:19-122`), portable con prueba de oráculo. |
| Rectángulo con radio, texto, líneas | Existe (`crates/pixpin-render/src/lienzo.rs:343-353`, `942`, `405`). |
| Fuente de UI | Assistant no viene con Windows; `--ui-font` cae a Segoe UI (`CSS/styles.scss:41-42`). |

### 8.3 Comparación con lo que ya tiene PixPin

**`crates/pixpin-ui/src/caja_herramientas.rs`**
- Hoy: caja **vertical**, al lado del contenido, botón **40 px**, hueco **2 px**, margen
  **6 px**, separación 12 px (`caja_herramientas.rs:15-20`, `98-144`). Excalidraw: barra
  **horizontal arriba al centro**, botón **36 px**, hueco **4 px**, padding **4 px**
  (sección 2.4).
- Se reutiliza: el patrón puro `colocar` / `rect_de` / `boton_en` / `destino` y su batería
  de pruebas (`:98-214`, `:216-480`), el `DestinoClic::Caja` que impide pintar detrás de la
  barra (`:194-214`).
- Falta: disposición horizontal centrada en la columna `2fr`; candado; separadores;
  desplegable «más herramientas»; distintivo de tecla; mover Deshacer/Rehacer al pie
  (`BotonCaja::Deshacer/Rehacer`, `:26-27`) y Zoom al pie; `Salir` no existe en
  Excalidraw.
- **Ojo con la Mano**: en PixPin `Herramienta::Mano` es «Seleccionar y mover lo ya
  dibujado» (`crates/pixpin-motor2d/src/gesto.rs:79-80`), o sea la **Selección (V/1)** de
  Excalidraw, no su Mano (H, desplazar lienzo, `C/Tools.tsx:70-74`). El cambio en curso de
  `apps/pixpin/src/caja_dibujo.rs` le pone `HAND_ICON`; debería ser `SELECTION_ICON`.
- Herramientas de PixPin sin equivalente en Excalidraw: Resaltador, Foco, Lupa, Cota,
  Escalar, EscalaGrafica (`caja_herramientas.rs:63-81`). Herramientas de Excalidraw sin
  equivalente en PixPin: Mano (panning), Rombo, Nota, Imagen, Marco, Embed, Láser, Bote,
  Lazo, Dibujo a forma.

**`apps/pixpin/src/caja_dibujo.rs`**
- En `HEAD`: fondo oscuro translúcido `rgba(0.12,0.12,0.14,0.92)` radio 8, activa azul
  `(0.25,0.45,0.85)` radio 6, letras en blanco (`git show HEAD:apps/pixpin/src/caja_dibujo.rs`,
  líneas 22-95). En el árbol de trabajo ya pinta iconos de 20 px en `Color::BLANCO`.
- Para copiar Excalidraw: fondo `--island-bg-color` `#ffffff` + `--shadow-island`, radio
  8 px; seleccionado `#e0dfff` radio 8 px con icono `#030064` (y relleno si `fillable`);
  reposo icono `#1b1b1f`; hover `#f1f0ff`; icono **16 px** en botón de 36 px (no 20 en 40).

**`crates/pixpin-ui/src/propiedades.rs`**
- Hoy: `Propiedad` = ColorTrazo, Relleno, Grosor, Estilo, Rugosidad, Opacidad, Fuente,
  TamanoTexto, PuntaFlecha (`propiedades.rs:18-29`), tablas por figura y herramienta
  (`:33-73`), e **intersección** de lo común en multiselección (`:79-91`).
- Se reutiliza: la idea de tabla pura por tipo y el orden fijo por enum (`:15-17`).
- Diferencias con Excalidraw:
  - Excalidraw usa **unión** (algún elemento o la herramienta,
    `C/shapeActionPredicates.ts:98-102`); PixPin usa intersección. Hay que decidir.
  - Orden de Excalidraw: Trazo, Fondo, Relleno, Grosor, Estilo, (Presión), Trazo a mano,
    Bordes, Tipo de flecha, Fuente, Tamaño, Alineación, Alineación vertical, Puntas,
    **Opacidad**, Capas, Alinear, Acciones (sección 3.1). En PixPin la opacidad va antes de
    la fuente.
  - Faltan en el enum: Fondo separado de Relleno (en PixPin `relleno: Option<ColorRgba>` es
    un color, sin estilo hachure/cross-hatch/solid, `crates/pixpin-motor2d/src/elemento.rs:136`),
    Bordes (roundness), Tipo de flecha, Alineación de texto, Presión (hoy solo la tecla V,
    `apps/pixpin/src/ventana_editor.rs:198-206`), Capas, Alinear/Distribuir, Acciones.
  - `EstiloTrazo` Sólido/Discontinuo/Punteado ya coincide con solid/dashed/dotted
    (`crates/pixpin-motor2d/src/elemento.rs:19-23`).

**`apps/pixpin/src/ventana_editor.rs`**
- La caja se coloca con `CajaHerramientas::colocar(area, area, …)` (`:427`, `:463`) y se
  pinta con `caja_dibujo::pintar_caja` sobre la vista de pantalla (`:1061-1072`); el clic se
  reparte con `DestinoClic` (`:559-576`). Se reutiliza tal cual cambiando la geometría.
- `BotonCaja::Color` no hace nada: «Sin paleta de colores en el editor todavía» (`:230-231`).
- **Conflictos de teclas** con Excalidraw: `tecla_a_herramienta` usa M/L/R/T/F/Q/B/A/E/G
  (`:173-187`) y `tecla_a_pluma` 1/2/3 y V (`:198-206`). En Excalidraw L=línea, R=rect,
  A=flecha, E=borrador, T=texto, Q=candado, B=bote, G=selector de fondo, F=marco, V=selección
  y 1-3 son herramientas (sección 7). Copiar la interfaz obliga a rehacer ese mapa.
- No hay panel de propiedades pintado en el editor: `propiedades::` no se usa fuera de su
  módulo (búsqueda en `apps/` y `crates/`).

### 8.4 Orden de trabajo sugerido

1. Tokens de tema (2.1) y sombra de isla en `pixpin-render`.
2. Completar el generador: `black` fijo, variantes `flip`/`theme`, máscara ensanchada.
3. Barra horizontal (2.4) reutilizando las pruebas de `caja_herramientas.rs`; corregir el
   icono de `Mano`; mapa de teclas de `Tools.tsx`.
4. Pie: zoom (− 60 px +) y deshacer/rehacer; ayuda a la derecha.
5. Panel de 200 px con las secciones de 3.1 que el modelo ya soporta (trazo, fondo, grosor,
   estilo, trazo a mano, opacidad, fuente, tamaño, puntas) y selector de color (4.2-4.3).
6. `applyDarkModeFilter` con oráculo, para el tema oscuro del lienzo.
