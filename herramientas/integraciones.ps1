# Integra PinDesk con Windows sin pedir administrador (todo en HKCU y en
# la carpeta del usuario), y lo quita con -Quitar. Lo llama instalar.ps1 en
# cada instalacion; tambien se puede correr suelto:
#
#   powershell -ExecutionPolicy Bypass -File herramientas\integraciones.ps1 [-Exe <pindesk.exe>]
#       [-Quitar] [-SoloMostrar] [-Raiz <ruta del registro>] [-EnviarA <carpeta>]
#
# Lo que pone (docs/integraciones.md):
#  1. El protocolo pindesk:// (HKCU\Software\Classes): un enlace
#     `pindesk://tarea?texto=…` abre `pindesk.exe --url "<enlace>"`, que lo
#     convierte en un pedido. Lo que no es inofensivo pregunta antes. Y el
#     pixpinmax:// de antes de PinDesk, que aun llevan avisos y enlaces
#     guardados. pixpin:// ya no se registra (puede ser del PixPin original
#     de DepthPixel): si lo tenia una version vieja, se suelta; si llega uno,
#     la app lo entiende igual (docs/formato-pindesk.md).
#  2. El menu contextual clasico del Explorador (clic derecho → «Mostrar mas
#     opciones» en Windows 11): «PinDesk ▸ Al chat / Pinear / Copiar texto
#     (OCR) / Enviar al movil / Preguntar al asistente». Con varios ficheros
#     elegidos, Explorer lanza una copia por fichero: `--juntar` hace que
#     se junten en un solo pedido (pindesk_shell::juntar).
#  3. «Enviar a» (accesos directos en SendTo): los mismos cinco; ahi
#     Windows ya pasa todos los ficheros de una vez.
#  4. El modulo de PowerShell (PinDesk.psm1) y la libreria de AutoHotkey
#     (pindesk.ahk), copiados junto al exe (y fuera los PixPin.psm1 y
#     pixpin.ahk de antes).
#
# -SoloMostrar escribe lo que haria y no toca nada. -Raiz y -EnviarA son
# para probar el script sobre una rama de prueba
# (p. ej. HKCU:\Software\PinDeskPrueba) y una carpeta temporal.

param(
    [string]$Exe = (Join-Path $env:LOCALAPPDATA 'Programs\PinDesk\pindesk.exe'),
    [switch]$Quitar,
    [switch]$SoloMostrar,
    [string]$Raiz = 'HKCU:\Software\Classes',
    [string]$EnviarA = (Join-Path $env:APPDATA 'Microsoft\Windows\SendTo')
)

$ErrorActionPreference = 'Stop'

# Las extensiones de imagen (las de pindesk_shell::orden::EXTENSIONES_IMAGEN):
# solo con ellas sale «Copiar texto (OCR)».
$imagenes = @('png', 'jpg', 'jpeg', 'bmp', 'gif', 'webp', 'tif')

# Las entradas: el verbo de la linea de ordenes y su rotulo. El orden es el
# del menu (el nombre de la clave lo ordena: 1chat, 2pinear...).
$entradas = @(
    @{ Clave = '1chat';      Verbo = 'chat';         Rotulo = 'Al chat';                 Lnk = 'PinDesk - al chat' },
    @{ Clave = '2pinear';    Verbo = 'pinear';       Rotulo = 'Pinear';                  Lnk = 'PinDesk - pinear' },
    @{ Clave = '3ocr';       Verbo = 'copiar-texto'; Rotulo = 'Copiar texto (OCR)';      Lnk = 'PinDesk - copiar texto (OCR)'; SoloImagenes = $true },
    @{ Clave = '4movil';     Verbo = 'al-movil';     Rotulo = 'Enviar al movil';         Lnk = 'PinDesk - enviar al movil' },
    @{ Clave = '5asistente'; Verbo = 'asistente';    Rotulo = 'Preguntar al asistente';  Lnk = 'PinDesk - preguntar al asistente' }
)
# Los de «Enviar a» de cuando la app se llamaba PixPin Max: se quitan siempre.
$lnkViejos = $entradas | ForEach-Object { $_.Lnk -replace '^PinDesk - ', 'PixPin - ' }

$protocolo = Join-Path $Raiz 'pindesk'
# Los de antes: pixpinmax:// se sigue registrando; pixpin:// solo se suelta.
$protocoloMax = Join-Path $Raiz 'pixpinmax'
$protocoloPixpin = Join-Path $Raiz 'pixpin'
$menu = Join-Path $Raiz '*\shell\PinDesk'
# La clave del menu de antes: se quita, para que no salgan dos submenus.
$menuViejo = Join-Path $Raiz '*\shell\PixPinMax'

function Pixpin-EsNuestro {
    # Si pixpin:// lo abre un pindesk.exe o el pixpinmax.exe de antes (lo
    # tomo una version vieja de este script): entonces hay que soltarlo.
    $c = Join-Path $protocoloPixpin 'shell\open\command'
    if (-not (Test-Path -LiteralPath $c)) { return $false }
    $orden = (Get-Item -LiteralPath $c).GetValue('')
    [bool]($orden -and $orden -match '(pindesk|pixpinmax)\.exe')
}

function Poner([string]$clave, [string]$nombre, [string]$valor) {
    # `-LiteralPath`: la clave `*` es un comodin para PowerShell.
    $mostrar = if ($nombre) { $nombre } else { '(predeterminado)' }
    if ($SoloMostrar) { Write-Host "  [$clave] $mostrar = $valor"; return }
    if (-not (Test-Path -LiteralPath $clave)) { New-Item -Path $clave -Force | Out-Null }
    if ($nombre) {
        New-ItemProperty -LiteralPath $clave -Name $nombre -Value $valor -PropertyType String -Force | Out-Null
    } else {
        Set-Item -LiteralPath $clave -Value $valor
    }
}

function Quitar-Clave([string]$clave) {
    if ($SoloMostrar) { Write-Host "  quitar [$clave]"; return }
    if (Test-Path -LiteralPath $clave) { Remove-Item -LiteralPath $clave -Recurse -Force }
}

function Lnk([string]$nombre) { Join-Path $EnviarA "$nombre.lnk" }

if ($Quitar) {
    Write-Host 'Quitando las integraciones de PinDesk:'
    Quitar-Clave $protocolo
    Quitar-Clave $protocoloMax
    # pixpin:// solo si es nuestro: el del PixPin original no se toca.
    if (Pixpin-EsNuestro) { Quitar-Clave $protocoloPixpin }
    Quitar-Clave $menu
    Quitar-Clave $menuViejo
    foreach ($n in @($entradas | ForEach-Object { $_.Lnk }) + $lnkViejos) {
        $l = Lnk $n
        if ($SoloMostrar) { Write-Host "  quitar $l" }
        elseif (Test-Path -LiteralPath $l) { Remove-Item -LiteralPath $l -Force }
    }
    Write-Host 'Hecho.'
    return
}

if (-not $SoloMostrar -and -not (Test-Path $Exe)) { throw "No esta $Exe" }
$comillado = '"' + $Exe + '"'

function Poner-Protocolo([string]$clave) {
    Write-Host "Protocolo $(Split-Path -Leaf $clave):// -> $Exe"
    Poner $clave '' 'URL:PinDesk'
    Poner $clave 'URL Protocol' ''
    Poner (Join-Path $clave 'DefaultIcon') '' "$comillado,0"
    Poner (Join-Path $clave 'shell\open\command') '' "$comillado --url `"%1`""
}

# pindesk:// es el de ahora; pixpinmax:// el de antes, que aun llevan avisos
# y enlaces guardados. pixpin:// ya no: si lo tomo una version vieja, se
# suelta para que sea del PixPin original de DepthPixel si esta.
Poner-Protocolo $protocolo
Poner-Protocolo $protocoloMax
if (Pixpin-EsNuestro) {
    Write-Host 'pixpin:// lo tenia PinDesk: se suelta (los enlaces son pindesk://)'
    Quitar-Clave $protocoloPixpin
}

Write-Host 'Menu contextual del Explorador (PinDesk >):'
Quitar-Clave $menuViejo
Poner $menu 'MUIVerb' 'PinDesk'
Poner $menu 'Icon' "$comillado,0"
Poner $menu 'SubCommands' ''
Poner $menu 'MultiSelectModel' 'Player'
foreach ($e in $entradas) {
    $c = Join-Path $menu "shell\$($e.Clave)"
    Poner $c 'MUIVerb' $e.Rotulo
    Poner $c 'MultiSelectModel' 'Player'
    if ($e.SoloImagenes) {
        $aplica = ($imagenes | ForEach-Object { "System.FileExtension:=.$_" }) -join ' OR '
        Poner $c 'AppliesTo' $aplica
    }
    Poner (Join-Path $c 'command') '' "$comillado --juntar $($e.Verbo) `"%1`""
}

Write-Host "Enviar a ($EnviarA):"
foreach ($n in $lnkViejos) {
    $l = Lnk $n
    if (-not $SoloMostrar -and (Test-Path -LiteralPath $l)) { Remove-Item -LiteralPath $l -Force }
}
foreach ($e in $entradas) {
    $l = Lnk $e.Lnk
    if ($SoloMostrar) { Write-Host "  $l -> $Exe $($e.Verbo)"; continue }
    New-Item -ItemType Directory -Force $EnviarA | Out-Null
    $s = (New-Object -ComObject WScript.Shell).CreateShortcut($l)
    $s.TargetPath = $Exe
    $s.Arguments = $e.Verbo
    $s.WorkingDirectory = Split-Path -Parent $Exe
    $s.IconLocation = "$Exe,0"
    $s.Description = "PinDesk: $($e.Rotulo)"
    $s.Save()
}

# El modulo de PowerShell y la libreria de AutoHotkey, junto al exe: asi
# `Import-Module "$env:LOCALAPPDATA\Programs\PinDesk\PinDesk.psm1"` siempre
# es el de la version instalada. Los de antes de PinDesk se quitan.
$dirExe = Split-Path -Parent $Exe
foreach ($f in @('PixPin.psm1', 'pixpin.ahk')) {
    $viejo = Join-Path $dirExe $f
    if ($SoloMostrar) { Write-Host "  quitar $viejo"; continue }
    Remove-Item $viejo -Force -ErrorAction SilentlyContinue
}
foreach ($f in @('PinDesk.psm1', 'pindesk.ahk')) {
    $de = Join-Path $PSScriptRoot $f
    if (-not (Test-Path $de)) { continue }
    if ($SoloMostrar) { Write-Host "  copiar $de -> $dirExe"; continue }
    Copy-Item $de (Join-Path $dirExe $f) -Force
}
if (-not $SoloMostrar) { Write-Host 'Integraciones puestas. Para quitarlas: integraciones.ps1 -Quitar' }
