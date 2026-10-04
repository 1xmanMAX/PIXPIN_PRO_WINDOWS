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
#
# Y si esta Flow Launcher (%APPDATA%\FlowLauncher\Plugins), pone al dia su
# plugin `pp` (apps/pixpin-lanzador): compila pixpin-lanzador.exe en la misma
# carpeta, lo copia con plugin.json e Images\ a Plugins\PixPin Max\ (quitando
# cualquier otra carpeta con el mismo ID: con dos, Flow no carga ninguna) y,
# si Flow estaba abierto, lo reinicia para que lo cargue.

param([string]$Origen)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$destinoDir = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax'
$destino = Join-Path $destinoDir 'pixpinmax.exe'
$compresor = 'pixpin-aligerar.exe'

if (-not $Origen) {
    # La mas nueva de todas las carpetas de compilacion (release, entrega-N...).
    # `target\release` es la de `cargo build --release` a secas; las demas,
    # `target\<carpeta>\release` (CARGO_TARGET_DIR = target\<carpeta>).
    $Origen = @((Get-Item (Join-Path $repo 'target'))) + @(Get-ChildItem (Join-Path $repo 'target') -Directory) |
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

# El plugin de Flow Launcher, de la MISMA carpeta de compilacion. Se compila
# si falta o si es mas viejo que pixpinmax.exe o que sus propias fuentes.
$lanzador = 'pixpin-lanzador.exe'
$fuenteLanzador = Join-Path $repo 'apps\pixpin-lanzador'
$pluginsFlow = Join-Path $env:APPDATA 'FlowLauncher\Plugins'
$origenLanzador = Join-Path $origenDir $lanzador
if (Test-Path $pluginsFlow) {
    $fuentesNuevas = Get-ChildItem $fuenteLanzador -Recurse -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $viejoLanzador = (Test-Path $origenLanzador) -and (
        ((Get-Item $origenLanzador).LastWriteTime -lt (Get-Item $Origen).LastWriteTime.AddHours(-1)) -or
        ($fuentesNuevas -and (Get-Item $origenLanzador).LastWriteTime -lt $fuentesNuevas.LastWriteTime))
    if (-not (Test-Path $origenLanzador) -or $viejoLanzador) {
        Write-Host "Compilando $lanzador en $(Split-Path -Parent $origenDir)..."
        $antes = $env:CARGO_TARGET_DIR
        try {
            $env:CARGO_TARGET_DIR = Split-Path -Parent $origenDir
            $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
            Push-Location $repo
            & cargo build --release -p pixpin-lanzador
        } catch {
            Write-Warning "No se pudo compilar ${lanzador}: $_"
        } finally {
            Pop-Location
            $env:CARGO_TARGET_DIR = $antes
        }
    }
}

function Instalar-PluginFlow {
    # Sin Flow, o sin el exe compilado, no hay nada que hacer.
    if (-not (Test-Path $pluginsFlow)) { return }
    if (-not (Test-Path $origenLanzador)) {
        Write-Warning "Falta $origenLanzador : no se instala el plugin de Flow Launcher"
        return
    }
    $fuente = Join-Path $fuenteLanzador 'plugin'
    $id = (Get-Content (Join-Path $fuente 'plugin.json') -Raw | ConvertFrom-Json).ID
    $destinoPlugin = Join-Path $pluginsFlow 'PixPin Max'
    # Todas las carpetas con nuestro ID, se llamen como se llamen.
    $mismas = @(Get-ChildItem $pluginsFlow -Directory | Where-Object {
        $pj = Join-Path $_.FullName 'plugin.json'
        try { (Test-Path $pj) -and ((Get-Content $pj -Raw | ConvertFrom-Json).ID -eq $id) } catch { $false }
    })
    $pares = @(
        @{ De = $origenLanzador; A = (Join-Path $destinoPlugin $lanzador) },
        @{ De = (Join-Path $fuente 'plugin.json'); A = (Join-Path $destinoPlugin 'plugin.json') }
    )
    Get-ChildItem (Join-Path $fuente 'Images') -File -ErrorAction SilentlyContinue | ForEach-Object {
        $pares += @{ De = $_.FullName; A = (Join-Path $destinoPlugin "Images\$($_.Name)") }
    }
    $alDia = ($mismas.Count -eq 1) -and ($mismas[0].FullName -eq $destinoPlugin)
    foreach ($p in $pares) {
        if (-not ((Test-Path $p.A) -and ((Get-FileHash $p.A).Hash -eq (Get-FileHash $p.De).Hash))) { $alDia = $false }
    }
    if ($alDia) { Write-Host "El plugin de Flow Launcher ya esta al dia: $destinoPlugin"; return }

    # Flow tiene el exe abierto (y lo mata con su Job al cerrarse).
    $flow = @(Get-Process Flow.Launcher -ErrorAction SilentlyContinue)
    if ($flow.Count -gt 0) {
        $flow | Stop-Process -Force
        for ($i = 0; $i -lt 25 -and (Get-Process Flow.Launcher -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 200 }
    }
    Get-Process pixpin-lanzador -ErrorAction SilentlyContinue | Stop-Process -Force
    $carpetas = @($mismas | ForEach-Object { $_.FullName }) + @($destinoPlugin) | Select-Object -Unique
    foreach ($c in $carpetas) {
        for ($i = 0; $i -lt 20 -and (Test-Path $c); $i++) {
            try { Remove-Item $c -Recurse -Force -ErrorAction Stop } catch { Start-Sleep -Milliseconds 500 }
        }
        if (Test-Path $c) { Write-Warning "No se pudo quitar $c (sigue en uso)" }
    }
    try {
        New-Item -ItemType Directory -Force (Join-Path $destinoPlugin 'Images') | Out-Null
        foreach ($p in $pares) { Copy-Item $p.De $p.A -Force -ErrorAction Stop }
        Write-Host "Plugin de Flow Launcher instalado: $destinoPlugin (escribe p)"
    } catch {
        Write-Warning "No se pudo instalar el plugin de Flow Launcher: $_"
    }
    # La palabra clave guardada en los ajustes de Flow manda sobre la de
    # plugin.json: se pone al dia con Flow cerrado (ya lo esta aqui).
    if (-not (Get-Process Flow.Launcher -ErrorAction SilentlyContinue)) {
        $palabra = Join-Path $repo 'apps\pixpin-lanzador\instalar-palabra-clave.ps1'
        if (Test-Path $palabra) {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $palabra
            if ($LASTEXITCODE -ne 0) { Write-Warning "No se pudo poner la palabra clave p en Flow (salida $LASTEXITCODE)" }
        }
    }
    if ($flow.Count -gt 0) {
        $exeFlow = Join-Path $env:LOCALAPPDATA 'FlowLauncher\Flow.Launcher.exe'
        if (Test-Path $exeFlow) { Start-Process $exeFlow; Write-Host "Flow Launcher reiniciado" }
    }
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

function Crear-Acceso {
    # El buscador de Windows encuentra las apps por su acceso directo en el
    # menu Inicio: sin el, escribir "PixPin" no sacaba nada. Se rehace en cada
    # instalacion (es barato) por si se borro o apunta a otro sitio.
    $programas = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $lnk = Join-Path $programas 'PixPin Max.lnk'
    try {
        $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
        $s.TargetPath = $destino
        $s.WorkingDirectory = $destinoDir
        $s.IconLocation = "$destino,0"
        $s.Description = 'PixPin Max: capturas, pines, chat y lienzo'
        $s.Save()
        Write-Host "En el menu Inicio: $lnk"
    } catch {
        Write-Warning "No se pudo crear el acceso del menu Inicio: $_"
    }
}

function Ofrecer-Predeterminada {
    # PixPin se registra sola para imagenes y videos al arrancar
    # (pixpin_shell::asociaciones), pero hacerla LA predeterminada solo lo
    # puede hacer el usuario: Windows deshace lo que escriba un programa. Se
    # le abre la pagina de PixPin Max en Configuracion UNA vez por cada
    # tanda de tipos nuevos; si ya lo es, o ya se le ofrecio, no se insiste.
    #
    # La marca guarda la tanda ofrecida ("tipos=2"). Cuando PixPin aprende
    # tipos nuevos se sube $tandaDeTipos y se vuelve a ofrecer UNA vez:
    #   1 = imagenes y videos
    #   2 = + audios (PixPinMax.Audio) y HEIC/HEIF/AVIF/JPEG XL/JFIF
    # Una marca vieja (solo traia la fecha) cuenta como tanda 1.
    $tandaDeTipos = 2
    $marca = Join-Path $destinoDir 'predeterminada-ofrecida.txt'
    $exts = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts'
    $elegido = {
        param($ext)
        (Get-ItemProperty "$exts\$ext\UserChoice" -ErrorAction SilentlyContinue).ProgId
    }
    $ya = ((& $elegido '.png') -eq 'PixPinMax.Imagen') -and
          ((& $elegido '.mp3') -eq 'PixPinMax.Audio')
    $ofrecida = 0
    if (Test-Path $marca) {
        $ofrecida = 1
        $linea = Get-Content $marca -ErrorAction SilentlyContinue |
            Where-Object { $_ -match '^tipos=(\d+)' } | Select-Object -First 1
        if ($linea -match '^tipos=(\d+)') { $ofrecida = [int]$Matches[1] }
    }
    if ($ya -or $ofrecida -ge $tandaDeTipos) { return }
    Start-Sleep -Seconds 2   # que la app recien abierta se registre antes
    Start-Process 'ms-settings:defaultapps?registeredAppUser=PixPin%20Max'
    Set-Content $marca @("tipos=$tandaDeTipos", (Get-Date -Format s))
    Write-Host 'Abierta Configuracion: pulsa "Establecer como predeterminada" en PixPin Max (fotos, videos y audios)'
}

# El plugin de Flow va aparte de la app: se pone al dia aunque PixPin ya sea
# la ultima, y un fallo suyo no impide instalar PixPin.
try { Instalar-PluginFlow } catch { Write-Warning "Plugin de Flow Launcher: $_" }

$corriendo = Get-Process pixpinmax -ErrorAction SilentlyContinue
$yaEsLaBuena = $igual -and $corriendo -and
    @($corriendo | Where-Object { $_.Path -ne $destino }).Count -eq 0
if ($yaEsLaBuena) {
    Write-Host "Ya corre la ultima version desde $destino"
    Crear-Acceso
    Ofrecer-Predeterminada
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
Crear-Acceso
Ofrecer-Predeterminada
