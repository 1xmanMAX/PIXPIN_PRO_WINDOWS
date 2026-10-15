# Integra PixPin Max con Windows sin pedir administrador (todo en HKCU y en
# la carpeta del usuario), y lo quita con -Quitar. Lo llama instalar.ps1 en
# cada instalacion; tambien se puede correr suelto:
#
#   powershell -ExecutionPolicy Bypass -File herramientas\integraciones.ps1 [-Exe <pixpinmax.exe>]
#       [-Quitar] [-SoloMostrar] [-Raiz <ruta del registro>] [-EnviarA <carpeta>]
#
# Lo que pone (docs/integraciones.md):
#  1. Los protocolos pixpinmax:// y pixpin:// (HKCU\Software\Classes): un
#     enlace `pixpinmax://tarea?texto=…` abre `pixpinmax.exe --url "<enlace>"`,
#     que lo convierte en un pedido. Lo que no es inofensivo pregunta antes.
#     pixpin:// (el de Android) solo si esta libre: el PixPin original de
#     DepthPixel lo usa, y no se le quita (salvo con -TomarPixpin).
#  2. El menu contextual clasico del Explorador (clic derecho → «Mostrar mas
#     opciones» en Windows 11): «PixPin ▸ Al chat / Pinear / Copiar texto
#     (OCR) / Enviar al movil / Preguntar al asistente». Con varios ficheros
#     elegidos, Explorer lanza una copia por fichero: `--juntar` hace que
#     se junten en un solo pedido (pixpin_shell::juntar).
#  3. «Enviar a» (accesos directos en SendTo): los mismos cinco; ahi
#     Windows ya pasa todos los ficheros de una vez.
#  4. El modulo de PowerShell (PixPin.psm1) y la libreria de AutoHotkey
#     (pixpin.ahk), copiados junto al exe.
#
# -SoloMostrar escribe lo que haria y no toca nada. -Raiz y -EnviarA son
# para probar el script sobre una rama de prueba
# (p. ej. HKCU:\Software\PixPinPrueba) y una carpeta temporal.

param(
    [string]$Exe = (Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax\pixpinmax.exe'),
    [switch]$Quitar,
    [switch]$SoloMostrar,
    [string]$Raiz = 'HKCU:\Software\Classes',
    [string]$EnviarA = (Join-Path $env:APPDATA 'Microsoft\Windows\SendTo'),
    # Tomar pixpin:// aunque sea de otro programa (el PixPin original).
    [switch]$TomarPixpin
)

$ErrorActionPreference = 'Stop'

# Las extensiones de imagen (las de pixpin_shell::orden::EXTENSIONES_IMAGEN):
# solo con ellas sale «Copiar texto (OCR)».
$imagenes = @('png', 'jpg', 'jpeg', 'bmp', 'gif', 'webp', 'tif')

# Las entradas: el verbo de la linea de ordenes y su rotulo. El orden es el
# del menu (el nombre de la clave lo ordena: 1chat, 2pinear...).
$entradas = @(
    @{ Clave = '1chat';      Verbo = 'chat';         Rotulo = 'Al chat';                 Lnk = 'PixPin - al chat' },
    @{ Clave = '2pinear';    Verbo = 'pinear';       Rotulo = 'Pinear';                  Lnk = 'PixPin - pinear' },
    @{ Clave = '3ocr';       Verbo = 'copiar-texto'; Rotulo = 'Copiar texto (OCR)';      Lnk = 'PixPin - copiar texto (OCR)'; SoloImagenes = $true },
    @{ Clave = '4movil';     Verbo = 'al-movil';     Rotulo = 'Enviar al movil';         Lnk = 'PixPin - enviar al movil' },
    @{ Clave = '5asistente'; Verbo = 'asistente';    Rotulo = 'Preguntar al asistente';  Lnk = 'PixPin - preguntar al asistente' }
)

$protocolo = Join-Path $Raiz 'pixpin'
$protocoloMax = Join-Path $Raiz 'pixpinmax'
$menu = Join-Path $Raiz '*\shell\PixPinMax'

function Protocolo-DeOtro {
    # La orden de pixpin:// si es de otro programa (no de un pixpinmax.exe).
    $c = Join-Path $protocolo 'shell\open\command'
    if (-not (Test-Path -LiteralPath $c)) { return $null }
    $orden = (Get-Item -LiteralPath $c).GetValue('')
    if (-not $orden -or $orden -match 'pixpinmax\.exe') { return $null }
    $orden
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
    Write-Host 'Quitando las integraciones de PixPin Max:'
    Quitar-Clave $protocoloMax
    # pixpin:// solo si es nuestro: el del PixPin original no se toca.
    if ((Test-Path -LiteralPath $protocolo) -and -not (Protocolo-DeOtro)) { Quitar-Clave $protocolo }
    Quitar-Clave $menu
    foreach ($e in $entradas) {
        $l = Lnk $e.Lnk
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
    Poner $clave '' 'URL:PixPin Max'
    Poner $clave 'URL Protocol' ''
    Poner (Join-Path $clave 'DefaultIcon') '' "$comillado,0"
    Poner (Join-Path $clave 'shell\open\command') '' "$comillado --url `"%1`""
}

# pixpinmax:// siempre es nuestro. pixpin:// (el de Android) solo si esta
# libre o ya era nuestro: el PixPin original de DepthPixel lo registra para
# si, y quitarselo romperia sus enlaces. -TomarPixpin lo toma igual.
Poner-Protocolo $protocoloMax
$deOtro = Protocolo-DeOtro
if ($deOtro -and -not $TomarPixpin) {
    Write-Host "pixpin:// es de otro programa ($deOtro): se deja; los enlaces del PC usan pixpinmax://"
} else {
    Poner-Protocolo $protocolo
}

Write-Host 'Menu contextual del Explorador (PixPin >):'
Poner $menu 'MUIVerb' 'PixPin'
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
foreach ($e in $entradas) {
    $l = Lnk $e.Lnk
    if ($SoloMostrar) { Write-Host "  $l -> $Exe $($e.Verbo)"; continue }
    New-Item -ItemType Directory -Force $EnviarA | Out-Null
    $s = (New-Object -ComObject WScript.Shell).CreateShortcut($l)
    $s.TargetPath = $Exe
    $s.Arguments = $e.Verbo
    $s.WorkingDirectory = Split-Path -Parent $Exe
    $s.IconLocation = "$Exe,0"
    $s.Description = "PixPin Max: $($e.Rotulo)"
    $s.Save()
}

# El modulo de PowerShell y la libreria de AutoHotkey, junto al exe: asi
# `Import-Module "$env:LOCALAPPDATA\Programs\PixPinMax\PixPin.psm1"` siempre
# es el de la version instalada.
$dirExe = Split-Path -Parent $Exe
foreach ($f in @('PixPin.psm1', 'pixpin.ahk')) {
    $de = Join-Path $PSScriptRoot $f
    if (-not (Test-Path $de)) { continue }
    if ($SoloMostrar) { Write-Host "  copiar $de -> $dirExe"; continue }
    Copy-Item $de (Join-Path $dirExe $f) -Force
}
if (-not $SoloMostrar) { Write-Host 'Integraciones puestas. Para quitarlas: integraciones.ps1 -Quitar' }
