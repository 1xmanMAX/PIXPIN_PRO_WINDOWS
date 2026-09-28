# Deja instalada la compilacion mas nueva de PixPin Max en un sitio FIJO y la
# abre. Uso:  powershell -ExecutionPolicy Bypass -File herramientas\instalar.ps1
#             [-Origen <ruta a pixpinmax.exe>]
#
# Por que un sitio fijo: la app escribe en HKCU\...\Run su PROPIA ruta cada vez
# que arranca. Si se abre desde target\entrega-N, el arranque con Windows queda
# atado a esa carpeta y la siguiente compilacion (entrega-N+1) nunca arranca
# sola. Copiando siempre al mismo sitio, la entrada del registro no cambia y
# Windows abre siempre la ultima version.
#
# Son DOS ficheros: pixpinmax.exe y pixpin-aligerar.exe, el compresor de PDF
# que pixpinmax lanza al meter un PDF al chat (apps/pixpin-aligerar). Van
# juntos y de la misma carpeta de compilacion: pixpinmax lo busca a su lado.
# Si el compresor no esta compilado, se compila ahi mismo; si aun asi falta,
# se instala pixpinmax solo y la app aligera los PDF con su plan B.
#
# Los datos no se mueven: sin pixpinmax.toml junto al .exe la app va en modo
# instalado y guarda todo en %APPDATA%\PixPinMax.

param([string]$Origen)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$destinoDir = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax'
$destino = Join-Path $destinoDir 'pixpinmax.exe'
$compresor = 'pixpin-aligerar.exe'

if (-not $Origen) {
    # La mas nueva de todas las carpetas de compilacion (release, entrega-N...).
    $Origen = Get-ChildItem (Join-Path $repo 'target') -Directory |
        ForEach-Object { Join-Path $_.FullName 'release\pixpinmax.exe' } |
        Where-Object { Test-Path $_ } |
        Sort-Object { (Get-Item $_).LastWriteTime } -Descending |
        Select-Object -First 1
}
if (-not $Origen -or -not (Test-Path $Origen)) { throw "No hay ningun pixpinmax.exe compilado" }
Write-Host "Origen:  $Origen ($((Get-Item $Origen).LastWriteTime))"

# El compresor, de la MISMA carpeta de compilacion. Si falta, o es mas viejo
# que pixpinmax.exe, se compila con esa misma carpeta de destino
# (target\<carpeta>\release -> CARGO_TARGET_DIR = target\<carpeta>).
$origenDir = Split-Path -Parent $Origen
$origenCompresor = Join-Path $origenDir $compresor
$viejo = (Test-Path $origenCompresor) -and
    ((Get-Item $origenCompresor).LastWriteTime -lt (Get-Item $Origen).LastWriteTime.AddHours(-1))
if (-not (Test-Path $origenCompresor) -or $viejo) {
    Write-Host "Compilando $compresor en $(Split-Path -Parent $origenDir)..."
    $antes = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = Split-Path -Parent $origenDir
        # cargo no esta en el PATH de una consola nueva en este equipo.
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        Push-Location $repo
        & cargo build --release -p pixpin-aligerar
    } catch {
        Write-Warning "No se pudo compilar ${compresor}: $_"
    } finally {
        Pop-Location
        $env:CARGO_TARGET_DIR = $antes
    }
}
if (-not (Test-Path $origenCompresor)) {
    Write-Warning "Falta $origenCompresor : se instala pixpinmax.exe solo y los PDF se aligeran con el plan B"
    $origenCompresor = $null
}

# Pares origen -> destino que hay que copiar.
$ficheros = @(@{ De = $Origen; A = $destino })
if ($origenCompresor) { $ficheros += @{ De = $origenCompresor; A = (Join-Path $destinoDir $compresor) } }

$igual = $true
foreach ($f in $ficheros) {
    if (-not ((Test-Path $f.A) -and ((Get-FileHash $f.A).Hash -eq (Get-FileHash $f.De).Hash))) {
        $igual = $false
    }
}

Add-Type -Namespace PixPin -Name W -MemberDefinition @'
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern System.IntPtr FindWindowExW(System.IntPtr p, System.IntPtr c, string cls, string win);
[DllImport("user32.dll")]
public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
'@

function Cerrar-PixPin {
    # La app vive en la bandeja e ignora el taskkill normal. Se le pide Salir
    # (WM_COMMAND 25) a su ventana de mensajes, que guarda lo pendiente.
    $h = [PixPin.W]::FindWindowExW([IntPtr](-3), [IntPtr]::Zero, 'PixPinMaxVentanaMensajes', $null)
    if ($h -ne [IntPtr]::Zero) { [void][PixPin.W]::PostMessageW($h, 0x0111, [IntPtr]25, [IntPtr]::Zero) }
    for ($i = 0; $i -lt 50 -and (Get-Process pixpinmax -ErrorAction SilentlyContinue); $i++) {
        Start-Sleep -Milliseconds 200
    }
    # Ultimo recurso tras 10 s: una copia colgada no puede impedir actualizar.
    Get-Process pixpinmax -ErrorAction SilentlyContinue | Stop-Process -Force
}

function Cerrar-Compresor {
    # El compresor se va solo al morir PixPin (se le cierra la entrada
    # estandar); por si acaso, se le espera un momento y luego se le cierra,
    # que un compresor a medias no deja nada roto: el PDF se queda como estaba.
    for ($i = 0; $i -lt 10 -and (Get-Process pixpin-aligerar -ErrorAction SilentlyContinue); $i++) {
        Start-Sleep -Milliseconds 200
    }
    Get-Process pixpin-aligerar -ErrorAction SilentlyContinue | Stop-Process -Force
}

$corriendo = Get-Process pixpinmax -ErrorAction SilentlyContinue
$yaEsLaBuena = $igual -and $corriendo -and
    @($corriendo | Where-Object { $_.Path -ne $destino }).Count -eq 0
if ($yaEsLaBuena) {
    Write-Host "Ya corre la ultima version desde $destino"
    exit 0
}

if ($corriendo) { Cerrar-PixPin }
if (-not $igual) {
    Cerrar-Compresor
    New-Item -ItemType Directory -Force $destinoDir | Out-Null
    # Windows suelta el .exe un momento DESPUES de que el proceso desaparece
    # de la lista (o lo retiene un antivirus): se reintenta hasta 15 s, cada
    # fichero. Si al final no se puede, se abre la version que habia para no
    # dejar al usuario sin la app.
    foreach ($f in $ficheros) {
        $copiado = $false
        for ($i = 0; $i -lt 30 -and -not $copiado; $i++) {
            try { Copy-Item $f.De $f.A -Force -ErrorAction Stop; $copiado = $true }
            catch { Start-Sleep -Milliseconds 500 }
        }
        if (-not $copiado) {
            if (Test-Path $destino) { Start-Process $destino }
            throw "No se pudo reemplazar $($f.A) (sigue en uso); se reabrio la version anterior"
        }
    }
}
Start-Process $destino
Write-Host "Instalada y abierta: $destino"
if ($origenCompresor) { Write-Host "Con el compresor de PDF: $(Join-Path $destinoDir $compresor)" }
