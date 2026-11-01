# Pone (o quita) la impresora "PinDesk": Imprimir -> PinDesk desde cualquier
# programa deja el PDF en la carpeta que vigila PinDesk, que lo abre en su
# lector como pin (crates/pixpin-shell/src/impresora.rs).
#
# Uso:
#   powershell -ExecutionPolicy Bypass -File herramientas\impresora-pindesk.ps1 -SoloMostrar
#   powershell -ExecutionPolicy Bypass -File herramientas\impresora-pindesk.ps1
#   powershell -ExecutionPolicy Bypass -File herramientas\impresora-pindesk.ps1 -Quitar
#   ... [-Raiz <carpeta de datos>]   (por defecto %APPDATA%\PinDesk; en modo
#                                     portable, la carpeta de pindesk.exe)
#
# La impresora de antes de PinDesk se llamaba "PixPin" (puerto PixPin.pdf en
# %APPDATA%\PixPinMax): al poner la nueva, o con -Quitar, se quita tambien.
#
# Como es: el controlador "Microsoft Print To PDF" que ya trae Windows (no se
# instala ningun controlador) con un PUERTO LOCAL que es un fichero:
# <Raiz>\cache\impresora\PinDesk.pdf. El puerto escribe siempre ahi; PinDesk lo
# mueve a <Raiz>\impresos\ en cuanto esta entero, y el puerto queda libre.
#
# Pide administrador UNA vez (crear un puerto y una impresora es cosa del
# administrador de impresion): la carpeta se crea antes como el usuario, y
# luego el mismo script se relanza elevado solo para Add-PrinterPort y
# Add-Printer. -SoloMostrar no cambia nada ni pide nada: dice lo que hay y lo
# que haria.
#
# PinDesk empieza a vigilar la carpeta al arrancar: tras instalar, cierra y
# abre PinDesk (o reinicia sesion).

param(
    [switch]$Quitar,
    [switch]$SoloMostrar,
    [string]$Raiz,
    # Uso interno: la copia elevada.
    [switch]$Elevado
)

$ErrorActionPreference = 'Stop'
$nombre = 'PinDesk'
$nombreViejo = 'PixPin'
$driver = 'Microsoft Print To PDF'
if (-not $Raiz) {
    # La de PinDesk; la de antes solo si aun no se ha mudado (la app la pasa
    # al abrirse: docs/formato-pindesk.md).
    $Raiz = Join-Path $env:APPDATA 'PinDesk'
    $vieja = Join-Path $env:APPDATA 'PixPinMax'
    if (-not (Test-Path $Raiz) -and (Test-Path $vieja) -and -not (Test-Path (Join-Path $vieja 'MOVIDO-A-PINDESK.txt'))) {
        $Raiz = $vieja
    }
}
$carpeta = Join-Path $Raiz 'cache\impresora'
$puerto = Join-Path $carpeta 'PinDesk.pdf'

function Es-Admin {
    $p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# --- Lo que hay ahora (no hace falta ser administrador para mirarlo) ---
$hayDriver = $null -ne (Get-PrinterDriver -Name $driver -ErrorAction SilentlyContinue)
$hayPuerto = $null -ne (Get-PrinterPort -Name $puerto -ErrorAction SilentlyContinue)
$hayImpresora = Get-Printer -Name $nombre -ErrorAction SilentlyContinue
$hayVieja = Get-Printer -Name $nombreViejo -ErrorAction SilentlyContinue
# Solo si es la nuestra: el controlador de PDF con un puerto PixPin.pdf.
if ($hayVieja -and $hayVieja.PortName -notlike '*\PixPin.pdf') { $hayVieja = $null }
$hayCarpeta = Test-Path $carpeta

Write-Host "Impresora '$nombre'"
Write-Host "  Controlador '$driver':  $(if ($hayDriver) {'esta'} else {'NO esta'})"
Write-Host "  Puerto (fichero):        $puerto  -> $(if ($hayPuerto) {'existe'} else {'no existe'})"
Write-Host "  Carpeta vigilada:        $carpeta -> $(if ($hayCarpeta) {'existe'} else {'no existe'})"
if ($hayImpresora) {
    Write-Host "  Impresora:               existe (puerto $($hayImpresora.PortName), controlador $($hayImpresora.DriverName))"
} else {
    Write-Host "  Impresora:               no existe"
}
if ($hayVieja) { Write-Host "  La de antes ('$nombreViejo'):  existe (puerto $($hayVieja.PortName)): se quita" }
Write-Host "  Administrador:           $(if (Es-Admin) {'si'} else {'no'})"

# --- Lo que se haria ---
$pasos = @()
if ($hayVieja) {
    $pasos += "Remove-Printer -Name '$nombreViejo'"
    $pasos += "Remove-PrinterPort -Name '$($hayVieja.PortName)'"
}
if ($Quitar) {
    if ($hayImpresora) { $pasos += "Remove-Printer -Name '$nombre'" }
    if ($hayPuerto) { $pasos += "Remove-PrinterPort -Name '$puerto'" }
    if (-not $pasos) { $pasos += '(nada: no esta puesta)' }
    $pasos += "(la carpeta $carpeta y los PDF de $(Join-Path $Raiz 'impresos') se quedan)"
} else {
    if (-not $hayCarpeta) { $pasos += "Crear la carpeta $carpeta (como el usuario)" }
    if (-not $hayDriver) {
        $pasos += "Enable-WindowsOptionalFeature -Online -FeatureName Printing-PrintToPDFServices-Features (trae el controlador)"
    }
    if (-not $hayPuerto) { $pasos += "Add-PrinterPort -Name '$puerto'" }
    if (-not $hayImpresora) {
        $pasos += "Add-Printer -Name '$nombre' -DriverName '$driver' -PortName '$puerto'"
    } elseif ($hayImpresora.PortName -ne $puerto) {
        $pasos += "Set-Printer -Name '$nombre' -PortName '$puerto' (ahora va a $($hayImpresora.PortName))"
    }
    if (-not $pasos) { $pasos += '(nada: ya esta puesta)' }
}
Write-Host ''
Write-Host $(if ($Quitar) { 'Para quitarla:' } else { 'Para ponerla:' })
$pasos | ForEach-Object { Write-Host "  - $_" }

if ($SoloMostrar) {
    Write-Host ''
    Write-Host '-SoloMostrar: no se ha cambiado nada.'
    return
}

# Solo quedan los pasos que no tocan nada: listo.
$necesitaAdmin = $pasos | Where-Object { $_ -match '^(Add-|Remove-|Set-Printer|Enable-)' }

# La carpeta, como el usuario (no como administrador): asi es suya y PinDesk
# puede mover y borrar lo que el puerto deje en ella.
if (-not $Quitar -and -not $hayCarpeta -and -not $Elevado) {
    New-Item -ItemType Directory -Force -Path $carpeta | Out-Null
    Write-Host "Carpeta creada: $carpeta"
}

if (-not $necesitaAdmin) { Write-Host 'Hecho.'; return }

if (-not (Es-Admin)) {
    if ($Elevado) { throw 'La copia elevada no tiene permisos de administrador.' }
    Write-Host ''
    Write-Host 'Pidiendo permiso de administrador (una vez)...'
    $argumentos = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Raiz', "`"$Raiz`"", '-Elevado')
    if ($Quitar) { $argumentos += '-Quitar' }
    $p = Start-Process -FilePath 'powershell.exe' -ArgumentList $argumentos -Verb RunAs -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "La parte de administrador fallo (codigo $($p.ExitCode))." }
    Write-Host 'Hecho. Cierra y abre PinDesk para que empiece a vigilar la carpeta.'
    return
}

# --- Ya como administrador ---
try {
    if ($hayVieja) {
        Remove-Printer -Name $nombreViejo
        Remove-PrinterPort -Name $hayVieja.PortName -ErrorAction SilentlyContinue
    }
    if ($Quitar) {
        if ($hayImpresora) { Remove-Printer -Name $nombre }
        if ($hayPuerto) { Remove-PrinterPort -Name $puerto }
    } else {
        if (-not $hayDriver) {
            Enable-WindowsOptionalFeature -Online -FeatureName 'Printing-PrintToPDFServices-Features' -NoRestart | Out-Null
        }
        if (-not $hayPuerto) { Add-PrinterPort -Name $puerto }
        if (-not $hayImpresora) {
            Add-Printer -Name $nombre -DriverName $driver -PortName $puerto
        } elseif ($hayImpresora.PortName -ne $puerto) {
            Set-Printer -Name $nombre -PortName $puerto
        }
    }
    Write-Host 'Hecho.'
} catch {
    Write-Host "Fallo: $_"
    if ($Elevado) { Start-Sleep -Seconds 8 }
    exit 1
}
