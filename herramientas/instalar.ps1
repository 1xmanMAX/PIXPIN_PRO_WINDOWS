# Deja instalada la compilacion mas nueva de PinDesk en un sitio FIJO y la
# abre. Uso:  powershell -ExecutionPolicy Bypass -File herramientas\instalar.ps1
#             [-Origen <ruta a pindesk.exe>]
#
# Por que un sitio fijo: la app escribe en HKCU\...\Run su PROPIA ruta cada vez
# que arranca. Si se abre desde target\entrega-N, el arranque con Windows queda
# atado a esa carpeta y la siguiente compilacion (entrega-N+1) nunca arranca
# sola. Copiando siempre al mismo sitio, la entrada del registro no cambia y
# Windows abre siempre la ultima version.
#
# Son DOS ficheros: pindesk.exe y pindesk-aligerar.exe, el compresor de PDF
# que pindesk lanza al meter un PDF al chat (apps/pindesk-aligerar). Van
# juntos y de la misma carpeta de compilacion: pindesk lo busca a su lado.
# Si el compresor no esta compilado, se compila ahi mismo; si aun asi falta,
# se instala pindesk solo y la app aligera los PDF con su plan B.
#
# Los datos no los mueve este script: sin pindesk.toml (ni el pindesk.toml
# de antes) junto al .exe la app va en modo instalado y guarda todo en
# %APPDATA%\PinDesk; al abrirse por primera vez pasa ahi el %APPDATA%\PixPinMax
# de antes (docs/formato-pindesk.md).
#
# La app se llamaba PixPin Max (pixpinmax.exe en Programs\PixPinMax, con sus
# ayudantes pixpin-*.exe). Si esta esa instalacion vieja, se migra: se cierra,
# se quitan sus .exe, su acceso del menu Inicio y su entrada de arranque
# (pasa a PinDesk), y sus PixPin.psm1 y pixpin.ahk (que ya no se instalan:
# ahora son PinDesk.psm1 y pindesk.ahk).
#
# Y si esta Flow Launcher (%APPDATA%\FlowLauncher\Plugins), pone al dia su
# plugin `p` (apps/pindesk-lanzador): compila pindesk-lanzador.exe en la misma
# carpeta, lo copia con plugin.json e Images\ a Plugins\PinDesk\ (quitando
# cualquier otra carpeta con el mismo ID: con dos, Flow no carga ninguna) y,
# si Flow estaba abierto, lo reinicia para que lo cargue.
#
# Y el puente de la extension del navegador (apps\pindesk-puente): se copia
# junto a pindesk.exe y se registra para Chrome y Edge en HKCU
# (herramientas\registrar-navegador.ps1; ver docs\integraciones-navegador.md).
# Y el servidor MCP (apps/pindesk-mcp, para Claude Desktop y Claude Code):
# pindesk-mcp.exe junto a pindesk.exe. La configuracion de Claude la pone el
# usuario (docs/integraciones-mcp.md); este script no la toca: si aun nombra
# el pixpin-mcp.exe de antes, deja ese nombre como copia del nuevo y avisa.

#
# Y pone las integraciones con Windows (herramientas\integraciones.ps1, todo
# en HKCU y sin administrador): el protocolo pindesk://, el menu del
# Explorador, «Enviar a» y los modulos de PowerShell y AutoHotkey.
# -SinIntegraciones las salta.

param([string]$Origen, [switch]$SinIntegraciones)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$destinoDir = Join-Path $env:LOCALAPPDATA 'Programs\PinDesk'
$destino = Join-Path $destinoDir 'pindesk.exe'
# La instalacion de cuando se llamaba PixPin Max.
$viejoDir = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax'
$compresor = 'pindesk-aligerar.exe'

if (-not $Origen) {
    # La mas nueva de todas las carpetas de compilacion (release, entrega-N...).
    # `target\release` es la de `cargo build --release` a secas; las demas,
    # `target\<carpeta>\release` (CARGO_TARGET_DIR = target\<carpeta>).
    $Origen = @((Get-Item (Join-Path $repo 'target'))) + @(Get-ChildItem (Join-Path $repo 'target') -Directory) |
        ForEach-Object { Join-Path $_.FullName 'release\pindesk.exe' } |
        Where-Object { Test-Path $_ } |
        Sort-Object { (Get-Item $_).LastWriteTime } -Descending |
        Select-Object -First 1
}
if (-not $Origen -or -not (Test-Path $Origen)) { throw "No hay ningun pindesk.exe compilado" }
Write-Host "Origen:  $Origen ($((Get-Item $Origen).LastWriteTime))"

# El compresor, de la MISMA carpeta de compilacion. Si falta, o es mas viejo
# que pindesk.exe, se compila con esa misma carpeta de destino
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
        & cargo build --release -p pindesk-aligerar
    } catch {
        Write-Warning "No se pudo compilar ${compresor}: $_"
    } finally {
        Pop-Location
        $env:CARGO_TARGET_DIR = $antes
    }
}
if (-not (Test-Path $origenCompresor)) {
    Write-Warning "Falta $origenCompresor : se instala pindesk.exe solo y los PDF se aligeran con el plan B"
    $origenCompresor = $null
}

# El puente de la extension del navegador (apps/pindesk-puente), igual que el
# compresor: de la misma carpeta de compilacion, compilado si falta o es
# viejo. Se instala junto a pindesk.exe y se registra en HKCU para Chrome y
# Edge (herramientas\registrar-navegador.ps1). La extension se carga a mano:
# docs\integraciones-navegador.md.
$puente = 'pindesk-puente.exe'
$origenPuente = Join-Path $origenDir $puente
$viejoPuente = (Test-Path $origenPuente) -and
    ((Get-Item $origenPuente).LastWriteTime -lt (Get-Item $Origen).LastWriteTime.AddHours(-1))
if (-not (Test-Path $origenPuente) -or $viejoPuente) {
    Write-Host "Compilando $puente en $(Split-Path -Parent $origenDir)..."
    $antes = $env:CARGO_TARGET_DIR
    try {
        $env:CARGO_TARGET_DIR = Split-Path -Parent $origenDir
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        Push-Location $repo
        & cargo build --release -p pindesk-puente
    } catch {
        Write-Warning "No se pudo compilar ${puente}: $_"
    } finally {
        Pop-Location
        $env:CARGO_TARGET_DIR = $antes
    }
}
if (-not (Test-Path $origenPuente)) {
    Write-Warning "Falta $origenPuente : la extension del navegador no podra hablar con PinDesk"
    $origenPuente = $null
}

function Registrar-Navegador {
    if (-not $origenPuente) { return }
    try {
        & (Join-Path $PSScriptRoot 'registrar-navegador.ps1') -Puente (Join-Path $destinoDir $puente)
    } catch {
        Write-Warning "No se pudo registrar el puente del navegador: $_"
    }
}

# El plugin de Flow Launcher, de la MISMA carpeta de compilacion. Se compila
# si falta o si es mas viejo que pindesk.exe o que sus propias fuentes.
$lanzador = 'pindesk-lanzador.exe'
$fuenteLanzador = Join-Path $repo 'apps\pindesk-lanzador'
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
            & cargo build --release -p pindesk-lanzador
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
    $destinoPlugin = Join-Path $pluginsFlow 'PinDesk'
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
    Get-Process pindesk-lanzador, pixpin-lanzador -ErrorAction SilentlyContinue | Stop-Process -Force
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
        $palabra = Join-Path $repo 'apps\pindesk-lanzador\instalar-palabra-clave.ps1'
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

# PinDesk Plantillas (apps/pindesk-plantillas, docs/plantillas.md): si se
# compilo en la misma carpeta (`cargo build --release -p pindesk-plantillas-app`),
# va junto a pindesk.exe con su acceso en el menu Inicio. Si no, no pasa nada.
$plantillas = 'pindesk-plantillas.exe'
$origenPlantillas = Join-Path $origenDir $plantillas
if (-not (Test-Path $origenPlantillas)) { $origenPlantillas = $null }

# Pares origen -> destino que hay que copiar.
$ficheros = @(@{ De = $Origen; A = $destino })
if ($origenPlantillas) { $ficheros += @{ De = $origenPlantillas; A = (Join-Path $destinoDir $plantillas) } }
if ($origenCompresor) { $ficheros += @{ De = $origenCompresor; A = (Join-Path $destinoDir $compresor) } }
if ($origenPuente) { $ficheros += @{ De = $origenPuente; A = (Join-Path $destinoDir $puente) } }

$igual = $true
foreach ($f in $ficheros) {
    if (-not ((Test-Path $f.A) -and ((Get-FileHash $f.A).Hash -eq (Get-FileHash $f.De).Hash))) {
        $igual = $false
    }
}

Add-Type -Namespace PinDesk -Name W -MemberDefinition @'
[DllImport("user32.dll", CharSet=CharSet.Unicode)]
public static extern System.IntPtr FindWindowExW(System.IntPtr p, System.IntPtr c, string cls, string win);
[DllImport("user32.dll")]
public static extern bool PostMessageW(System.IntPtr h, uint m, System.IntPtr w, System.IntPtr l);
'@

function Cerrar-PinDesk {
    # La app vive en la bandeja e ignora el taskkill normal. Se le pide Salir
    # (WM_COMMAND 25) a su ventana de mensajes, que guarda lo pendiente. La
    # clase es PinDeskVentanaMensajes o, en un pixpinmax.exe de antes,
    # PixPinMaxVentanaMensajes; el proceso puede ser pindesk o pixpinmax.
    # [NullString]::Value y no $null: PowerShell pasa $null a un string de
    # C# como "" y Windows buscaba la ventana de TITULO vacio, que no existe;
    # nunca la encontraba y la app acababa siempre cerrada a la fuerza.
    $h = [PinDesk.W]::FindWindowExW([IntPtr](-3), [IntPtr]::Zero, 'PinDeskVentanaMensajes', [NullString]::Value)
    if ($h -eq [IntPtr]::Zero) {
        $h = [PinDesk.W]::FindWindowExW([IntPtr](-3), [IntPtr]::Zero, 'PixPinMaxVentanaMensajes', [NullString]::Value)
    }
    if ($h -ne [IntPtr]::Zero) { Write-Host 'Cerrando PinDesk con Salir (guarda lo pendiente)...' }
    else { Write-Warning 'No se encontro la ventana de PinDesk: si sigue abierta se cerrara a la fuerza' }
    if ($h -ne [IntPtr]::Zero) { [void][PinDesk.W]::PostMessageW($h, 0x0111, [IntPtr]25, [IntPtr]::Zero) }
    for ($i = 0; $i -lt 50 -and (Get-Process pindesk, pixpinmax -ErrorAction SilentlyContinue); $i++) {
        Start-Sleep -Milliseconds 200
    }
    # Ultimo recurso tras 10 s: una copia colgada no puede impedir actualizar.
    if (Get-Process pindesk, pixpinmax -ErrorAction SilentlyContinue) { Write-Warning 'PinDesk no se cerro en 10 s: se cierra a la fuerza' }
    Get-Process pindesk, pixpinmax -ErrorAction SilentlyContinue | Stop-Process -Force
}

function Cerrar-Compresor {
    # El compresor se va solo al morir PinDesk (se le cierra la entrada
    # estandar); por si acaso, se le espera un momento y luego se le cierra,
    # que un compresor a medias no deja nada roto: el PDF se queda como estaba.
    for ($i = 0; $i -lt 10 -and (Get-Process pindesk-aligerar, pixpin-aligerar -ErrorAction SilentlyContinue); $i++) {
        Start-Sleep -Milliseconds 200
    }
    Get-Process pindesk-aligerar, pixpin-aligerar -ErrorAction SilentlyContinue | Stop-Process -Force
    # PinDesk Plantillas abierta no deja sustituir su .exe: se cierra (lo que
    # tuviera sin guardar se pierde; avisa al instalar a mano).
    if (Get-Process pindesk-plantillas -ErrorAction SilentlyContinue) {
        Write-Warning 'Se cierra PinDesk Plantillas para ponerla al dia'
        Get-Process pindesk-plantillas -ErrorAction SilentlyContinue | Stop-Process -Force
    }
}

function Crear-Acceso {
    # El buscador de Windows encuentra las apps por su acceso directo en el
    # menu Inicio: sin el, escribir "PinDesk" no sacaba nada. Se rehace en
    # cada instalacion (es barato) por si se borro o apunta a otro sitio. El
    # de cuando se llamaba PixPin Max se quita.
    $programas = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $lnk = Join-Path $programas 'PinDesk.lnk'
    Remove-Item (Join-Path $programas 'PixPin Max.lnk') -Force -ErrorAction SilentlyContinue
    try {
        $s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
        $s.TargetPath = $destino
        $s.WorkingDirectory = $destinoDir
        $s.IconLocation = "$destino,0"
        $s.Description = 'PinDesk: capturas, pines, chat y lienzo'
        $s.Save()
        Write-Host "En el menu Inicio: $lnk"
    } catch {
        Write-Warning "No se pudo crear el acceso del menu Inicio: $_"
    }
    # PinDesk Plantillas (nombre provisional: nucleo::NOMBRE_PRODUCTO).
    $exePlantillas = Join-Path $destinoDir $plantillas
    if (Test-Path $exePlantillas) {
        try {
            $s = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $programas 'PinDesk Plantillas.lnk'))
            $s.TargetPath = $exePlantillas
            $s.WorkingDirectory = $destinoDir
            $s.IconLocation = "$exePlantillas,0"
            $s.Description = 'PinDesk Plantillas: diarios, planificadores y calendarios por bloques'
            $s.Save()
        } catch {
            Write-Warning "No se pudo crear el acceso de PinDesk Plantillas: $_"
        }
    }
}

function Ofrecer-Predeterminada {
    # PinDesk se registra sola para imagenes y videos al arrancar
    # (pindesk_shell::asociaciones), pero hacerla LA predeterminada solo lo
    # puede hacer el usuario: Windows deshace lo que escriba un programa. Se
    # le abre la pagina de PinDesk en Configuracion UNA vez por cada
    # tanda de tipos nuevos; si ya lo es, o ya se le ofrecio, no se insiste.
    #
    # La marca guarda la tanda ofrecida ("tipos=2"). Cuando PinDesk aprende
    # tipos nuevos se sube $tandaDeTipos y se vuelve a ofrecer UNA vez:
    #   1 = imagenes y videos
    #   2 = + audios y HEIC/HEIF/AVIF/JPEG XL/JFIF
    #   3 = los ProgID pasan de PixPinMax.* a PinDesk.* (nov-2026): la
    #       eleccion de antes ya no vale y hay que volver a elegir
    # Una marca vieja (solo traia la fecha) cuenta como tanda 1.
    $tandaDeTipos = 3
    $marca = Join-Path $destinoDir 'predeterminada-ofrecida.txt'
    # La de la instalacion vieja vale igual: ya se le ofrecio.
    $marcaVieja = Join-Path $viejoDir 'predeterminada-ofrecida.txt'
    if (-not (Test-Path $marca) -and (Test-Path $marcaVieja)) { Copy-Item $marcaVieja $marca -ErrorAction SilentlyContinue }
    $exts = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts'
    $elegido = {
        param($ext)
        (Get-ItemProperty "$exts\$ext\UserChoice" -ErrorAction SilentlyContinue).ProgId
    }
    # Los ProgID son PinDesk.* desde nov-2026; los PixPinMax.* de antes los
    # borra la app al arrancar, asi que un UserChoice con ellos ya no vale.
    $ya = ((& $elegido '.png') -eq 'PinDesk.Imagen') -and
          ((& $elegido '.mp3') -eq 'PinDesk.Audio')
    $ofrecida = 0
    if (Test-Path $marca) {
        $ofrecida = 1
        $linea = Get-Content $marca -ErrorAction SilentlyContinue |
            Where-Object { $_ -match '^tipos=(\d+)' } | Select-Object -First 1
        if ($linea -match '^tipos=(\d+)') { $ofrecida = [int]$Matches[1] }
    }
    if ($ya -or $ofrecida -ge $tandaDeTipos) { return }
    Start-Sleep -Seconds 2   # que la app recien abierta se registre antes
    Start-Process 'ms-settings:defaultapps?registeredAppUser=PinDesk'
    Set-Content $marca @("tipos=$tandaDeTipos", (Get-Date -Format s))
    Write-Host 'Abierta Configuracion: pulsa "Establecer como predeterminada" en PinDesk (fotos, videos y audios)'
}

# El plugin de Flow va aparte de la app: se pone al dia aunque PinDesk ya sea
# la ultima, y un fallo suyo no impide instalar PinDesk.
try { Instalar-PluginFlow } catch { Write-Warning "Plugin de Flow Launcher: $_" }

# Las integraciones apuntan a $destino, que no cambia de una version a otra;
# se ponen en cada instalacion por si se quitaron. Si el exe aun no esta
# instalado, se ponen al final (abajo), cuando ya lo este.
function Poner-Integraciones {
    if ($SinIntegraciones -or $script:integracionesPuestas -or -not (Test-Path $destino)) { return }
    $script:integracionesPuestas = $true
    try { & (Join-Path $PSScriptRoot 'integraciones.ps1') -Exe $destino }
    catch { Write-Warning "Integraciones con Windows: $_" }
}
Poner-Integraciones

function Instalar-Mcp {
    # El servidor MCP para Claude Desktop y Claude Code (apps/pindesk-mcp,
    # docs/integraciones-mcp.md), junto a pindesk.exe. Se compila en la
    # misma carpeta de compilacion si falta o es mas viejo que sus fuentes.
    # NO toca la configuracion de Claude: eso lo hace el usuario (ver docs).
    $mcp = 'pindesk-mcp.exe'
    $origenMcp = Join-Path $origenDir $mcp
    $destinoMcp = Join-Path $destinoDir $mcp
    $fuentesMcp = Get-ChildItem (Join-Path $repo 'apps\pindesk-mcp') -Recurse -File |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    $viejoMcp = (Test-Path $origenMcp) -and (
        ((Get-Item $origenMcp).LastWriteTime -lt (Get-Item $Origen).LastWriteTime.AddHours(-1)) -or
        ($fuentesMcp -and (Get-Item $origenMcp).LastWriteTime -lt $fuentesMcp.LastWriteTime))
    if (-not (Test-Path $origenMcp) -or $viejoMcp) {
        Write-Host "Compilando $mcp en $(Split-Path -Parent $origenDir)..."
        $antes = $env:CARGO_TARGET_DIR
        try {
            $env:CARGO_TARGET_DIR = Split-Path -Parent $origenDir
            $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
            Push-Location $repo
            & cargo build --release -p pindesk-mcp
        } catch {
            Write-Warning "No se pudo compilar ${mcp}: $_"
        } finally {
            Pop-Location
            $env:CARGO_TARGET_DIR = $antes
        }
    }
    if (-not (Test-Path $origenMcp)) { Write-Warning "Falta $origenMcp : no se instala el servidor MCP"; return }
    if ((Test-Path $destinoMcp) -and ((Get-FileHash $destinoMcp).Hash -eq (Get-FileHash $origenMcp).Hash)) {
        Write-Host "El servidor MCP ya esta al dia: $destinoMcp"
        return
    }
    # Un cliente (Claude Desktop) puede tenerlo abierto: no guarda nada, se
    # cierra y el cliente lo vuelve a arrancar al usarlo.
    Get-Process pindesk-mcp, pixpin-mcp -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $destinoMcp -or $_.Path -eq (Join-Path $destinoDir 'pixpin-mcp.exe') } |
        Stop-Process -Force
    New-Item -ItemType Directory -Force $destinoDir | Out-Null
    for ($i = 0; $i -lt 20; $i++) {
        try { Copy-Item $origenMcp $destinoMcp -Force -ErrorAction Stop; break } catch { Start-Sleep -Milliseconds 500 }
    }
    if ((Test-Path $destinoMcp) -and ((Get-FileHash $destinoMcp).Hash -eq (Get-FileHash $origenMcp).Hash)) {
        Write-Host "Servidor MCP instalado: $destinoMcp (ver docs\integraciones-mcp.md)"
    } else {
        Write-Warning "No se pudo copiar $destinoMcp (sigue en uso)"
    }
    Quitar-McpViejo $origenMcp
}

function Quitar-McpViejo([string]$origenMcp) {
    # El pixpin-mcp.exe de antes de PinDesk: si la configuracion de Claude
    # (Desktop o Code) aun lo nombra, se deja como copia del nuevo para no
    # romperla y se avisa; si no, se quita.
    $viejoMcp = Join-Path $destinoDir 'pixpin-mcp.exe'
    if (-not (Test-Path $viejoMcp)) { return }
    $configs = @((Join-Path $env:APPDATA 'Claude\claude_desktop_config.json'), (Join-Path $env:USERPROFILE '.claude.json'))
    $loNombra = @($configs | Where-Object { (Test-Path $_) -and (Select-String -Path $_ -SimpleMatch 'pixpin-mcp' -Quiet) }).Count -gt 0
    try {
        if ($loNombra) {
            Copy-Item $origenMcp $viejoMcp -Force -ErrorAction Stop
            Write-Warning "Tu configuracion de Claude usa pixpin-mcp.exe: cambiala a pindesk-mcp.exe (docs\integraciones-mcp.md)"
        } else {
            Remove-Item $viejoMcp -Force -ErrorAction Stop
        }
    } catch { Write-Warning "No se pudo poner al dia $viejoMcp (sigue en uso)" }
}
try { Instalar-Mcp } catch { Write-Warning "Servidor MCP: $_" }

function Migrar-PixPinMax {
    # La instalacion de cuando la app se llamaba PixPin Max: fuera sus .exe
    # (ya no se abren: el arranque, Abrir con y el menu Inicio pasan a
    # PinDesk) y su entrada de arranque con Windows, que pasa a «PinDesk»
    # (la app tambien lo hace al abrirse). Si en esa carpeta hay un
    # pindesk.toml es una copia PORTABLE con sus datos: no se toca nada.
    # Y en la carpeta nueva, los ayudantes con el nombre de antes (pixpin-*)
    # y el modulo de PowerShell y el script de AutoHotkey viejos.
    foreach ($exe in 'pixpin-aligerar.exe', 'pixpin-lanzador.exe', 'pixpin-puente.exe') {
        $f = Join-Path $destinoDir $exe
        if (Test-Path $f) {
            try { Remove-Item $f -Force -ErrorAction Stop } catch { Write-Warning "No se pudo quitar $f (sigue en uso)" }
        }
    }
    foreach ($f in 'PixPin.psm1', 'pixpin.ahk') {
        Remove-Item (Join-Path $destinoDir $f) -Force -ErrorAction SilentlyContinue
    }
    if (-not (Test-Path $viejoDir)) { return }
    if (Test-Path (Join-Path $viejoDir 'pixpinmax.toml')) {
        Write-Warning "$viejoDir tiene un pixpinmax.toml (modo portable): se deja como esta"
        return
    }
    $run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
    $orden = (Get-ItemProperty $run -Name PixPinMax -ErrorAction SilentlyContinue).PixPinMax
    if ($orden) {
        New-ItemProperty $run -Name PinDesk -Value "`"$destino`"" -PropertyType String -Force | Out-Null
        Remove-ItemProperty $run -Name PixPinMax -ErrorAction SilentlyContinue
    }
    foreach ($exe in 'pixpinmax.exe', 'pixpin-aligerar.exe', 'pixpin-lanzador.exe', 'pixpin-puente.exe', 'pixpin-mcp.exe') {
        $f = Join-Path $viejoDir $exe
        if (Test-Path $f) {
            try { Remove-Item $f -Force -ErrorAction Stop } catch { Write-Warning "No se pudo quitar $f (sigue en uso)" }
        }
    }
    # El modulo y el script de antes ya no se instalan (ahora PinDesk.psm1 y
    # pindesk.ahk, en Programs\PinDesk): quien los importaba desde aqui
    # tiene que cambiar la ruta de su perfil (docs/integraciones-windows.md).
    foreach ($f in @('PixPin.psm1', 'pixpin.ahk')) {
        $viejoF = Join-Path $viejoDir $f
        if (Test-Path $viejoF) {
            Remove-Item $viejoF -Force -ErrorAction SilentlyContinue
            Write-Warning "Quitado $viejoF : ahora es $(Join-Path $destinoDir ($f -creplace 'PixPin', 'PinDesk' -creplace 'pixpin', 'pindesk'))"
        }
    }
    Write-Host "Migrada la instalacion de PixPin Max ($viejoDir) a $destinoDir"
}

$corriendo = Get-Process pindesk, pixpinmax -ErrorAction SilentlyContinue
$yaEsLaBuena = $igual -and $corriendo -and
    @($corriendo | Where-Object { $_.Path -ne $destino }).Count -eq 0
if ($yaEsLaBuena) {
    Write-Host "Ya corre la ultima version desde $destino"
    Migrar-PixPinMax
    Crear-Acceso
    Registrar-Navegador
    Ofrecer-Predeterminada
    exit 0
}

if ($corriendo) { Cerrar-PinDesk }
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
Migrar-PixPinMax
Start-Process $destino
Write-Host "Instalada y abierta: $destino"
if ($origenCompresor) { Write-Host "Con el compresor de PDF: $(Join-Path $destinoDir $compresor)" }
Registrar-Navegador
Crear-Acceso
Ofrecer-Predeterminada
Poner-Integraciones
