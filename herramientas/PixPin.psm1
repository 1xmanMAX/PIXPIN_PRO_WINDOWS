# PixPin.psm1 — pedirle cosas a PixPin Max desde PowerShell.
#
#   Import-Module "$env:LOCALAPPDATA\Programs\PixPinMax\PixPin.psm1"
#   New-PixPinTarea "llamar al ingeniero" -Proyecto Obra
#   Get-ChildItem *.png | Send-PixPinChat -Texto "fotos de hoy"
#   Get-PixPinTexto .\captura.png | Set-Clipboard
#   Get-Process | Select-Object Name, CPU -First 10 | Out-PixPin -Tabla "Procesos"
#
# Todo va por la linea de ordenes de pixpinmax.exe (docs/integraciones.md):
# este modulo no habla con la app por su cuenta. Los pedidos se le pasan por
# la entrada estandar (`--pedido-archivo -`), asi un texto con comillas,
# saltos de linea o acentos llega entero sin pelearse con las reglas de
# comillas de Windows.
#
# Cada funcion devuelve lo que la orden escribe (el texto del OCR, la ayuda)
# y deja el codigo de salida en $LASTEXITCODE: 0 aceptado, 1 fallo local,
# 2 version, 3 no se entiende, 4 PixPin no esta abierta, 5 no contesto,
# 6 rechazado. Si no es 0, ademas escribe un error (Write-Error).
#
# El ejecutable: $env:PIXPIN_EXE, o el que esta junto a este modulo, o el
# instalado en %LOCALAPPDATA%\Programs\PixPinMax.

Set-StrictMode -Version 2

function Get-PixPinExe {
    <# .SYNOPSIS Donde esta pixpinmax.exe. #>
    if ($env:PIXPIN_EXE -and (Test-Path $env:PIXPIN_EXE)) { return $env:PIXPIN_EXE }
    $junto = Join-Path $PSScriptRoot 'pixpinmax.exe'
    if (Test-Path $junto) { return $junto }
    $instalado = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax\pixpinmax.exe'
    if (Test-Path $instalado) { return $instalado }
    throw 'No se encuentra pixpinmax.exe (pon su ruta en $env:PIXPIN_EXE)'
}

function Invoke-PixPin {
    <#
    .SYNOPSIS
    Corre pixpinmax.exe con esos argumentos y espera a que acabe.
    .DESCRIPTION
    pixpinmax.exe es una aplicacion de ventanas: lanzada a secas, la consola no
    la espera ni ve lo que escribe. Aqui se lanza con su salida redirigida y se
    espera, asi el texto y el codigo de salida llegan.
    .EXAMPLE
    Invoke-PixPin capturar
    .EXAMPLE
    Invoke-PixPin ayuda
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0, ValueFromRemainingArguments = $true)][string[]]$Argumentos,
        # Lo que se le da por la entrada estandar (un pedido JSON).
        [string]$Entrada
    )
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = Get-PixPinExe
    $info.Arguments = (@($Argumentos) | Where-Object { $_ -ne $null } | ForEach-Object { ConvertTo-PixPinArgumento $_ }) -join ' '
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.RedirectStandardInput = $true
    $info.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $info.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $p = [System.Diagnostics.Process]::Start($info)
    if ($Entrada) {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($Entrada)
        $p.StandardInput.BaseStream.Write($bytes, 0, $bytes.Length)
    }
    $p.StandardInput.Close()
    $salida = $p.StandardOutput.ReadToEndAsync()
    $errores = $p.StandardError.ReadToEnd()
    $p.WaitForExit()
    $global:LASTEXITCODE = $p.ExitCode
    $texto = $salida.Result
    if ($p.ExitCode -ne 0) {
        $motivo = ($errores.Trim(), "pixpinmax salio con $($p.ExitCode)" | Where-Object { $_ } | Select-Object -First 1)
        Write-Error -Message $motivo -Category InvalidOperation
    }
    if ($texto) { $texto.TrimEnd("`r", "`n") }
}

function ConvertTo-PixPinArgumento([string]$a) {
    # Las reglas de CommandLineToArgvW: entre comillas si hace falta, las
    # comillas de dentro con barra y las barras que las preceden dobladas.
    if ($a -ne '' -and $a -notmatch '[\s"]') { return $a }
    $s = '"'
    $barras = 0
    foreach ($c in $a.ToCharArray()) {
        if ($c -eq '\') { $barras++; continue }
        if ($c -eq '"') { $s += ('\' * ($barras * 2 + 1)) + '"'; $barras = 0; continue }
        $s += ('\' * $barras) + $c
        $barras = 0
    }
    $s + ('\' * ($barras * 2)) + '"'
}

function Send-PixPin {
    <#
    .SYNOPSIS
    Manda un pedido del protocolo (docs/protocolo-pedidos.md) a PixPin.
    .EXAMPLE
    Send-PixPin -Accion anadir_tarea -Campos @{ texto = 'pan'; proyecto = 'pr-1' }
    .EXAMPLE
    Send-PixPin -Json '{"pixpin":1,"accion":"capturar"}'
    #>
    [CmdletBinding(DefaultParameterSetName = 'Accion')]
    param(
        [Parameter(ParameterSetName = 'Accion', Mandatory = $true, Position = 0)][string]$Accion,
        [Parameter(ParameterSetName = 'Accion', Position = 1)][hashtable]$Campos = @{},
        [Parameter(ParameterSetName = 'Json', Mandatory = $true)][string]$Json,
        # Solo devolver el JSON, sin mandarlo.
        [switch]$ComoJson,
        # Si PixPin no esta abierta, no abrirla (sale con 4).
        [switch]$SinArrancar
    )
    if ($PSCmdlet.ParameterSetName -eq 'Accion') {
        $pedido = [ordered]@{ pixpin = 1; accion = $Accion }
        foreach ($k in $Campos.Keys) { $pedido[$k] = $Campos[$k] }
        $Json = $pedido | ConvertTo-Json -Depth 10 -Compress
    }
    if ($ComoJson) { return $Json }
    $a = @('--pedido-archivo', '-')
    if ($SinArrancar) { $a += '--sin-arrancar' }
    Invoke-PixPin -Argumentos $a -Entrada $Json
}

function Resolve-PixPinRuta([string[]]$Ruta) {
    foreach ($r in $Ruta) { (Resolve-Path -LiteralPath $r -ErrorAction Stop).ProviderPath }
}

function Add-PixPinCampoProyecto([hashtable]$c, [string]$Proyecto) {
    # Un nombre lo cambia por su id la propia orden.
    if ($Proyecto) { $c.proyecto = $Proyecto }
    $c
}

function New-PixPinTarea {
    <#
    .SYNOPSIS
    Apunta una tarea (sin proyecto ni lista, en el Inbox). Acepta la tuberia:
    cada linea, una tarea.
    .EXAMPLE
    'pan', 'leche' | New-PixPinTarea -Proyecto Casa
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true)][string]$Texto,
        [string]$Proyecto,
        # El id de la lista (mensaje) donde va.
        [string]$Lista,
        [switch]$ComoJson
    )
    process {
        $c = Add-PixPinCampoProyecto @{ texto = $Texto } $Proyecto
        if ($Lista) { $c.codigo = $Lista }
        Send-PixPin -Accion anadir_tarea -Campos $c -ComoJson:$ComoJson
    }
}

function New-PixPinNota {
    <# .SYNOPSIS Abre una nota nueva empezada con ese texto. La tuberia se junta en una nota. #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0, ValueFromPipeline = $true)][string]$Texto,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    begin { $lineas = New-Object System.Collections.Generic.List[string] }
    process { if ($Texto) { $lineas.Add($Texto) } }
    end {
        $c = Add-PixPinCampoProyecto @{} $Proyecto
        if ($lineas.Count) { $c.texto = $lineas -join "`n" }
        Send-PixPin -Accion nota_nueva -Campos $c -ComoJson:$ComoJson
    }
}

function Send-PixPinChat {
    <#
    .SYNOPSIS
    Escribe en el chat (sin proyecto, en Mensajes guardados) un texto y
    ficheros. Los ficheros pueden llegar por la tuberia (Get-ChildItem).
    .EXAMPLE
    Get-ChildItem .\fotos\*.jpg | Send-PixPinChat -Texto 'Avance de obra' -Proyecto Obra
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)][string]$Texto = '',
        [Parameter(ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Archivo,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    begin { $todos = New-Object System.Collections.Generic.List[string] }
    process { if ($Archivo) { foreach ($r in (Resolve-PixPinRuta $Archivo)) { $todos.Add($r) } } }
    end {
        $imagen = '\.(png|jpe?g|bmp|gif|webp|tif)$'
        $c = Add-PixPinCampoProyecto @{ texto = $Texto } $Proyecto
        $img = @($todos | Where-Object { $_ -match $imagen })
        $otros = @($todos | Where-Object { $_ -notmatch $imagen })
        if ($img.Count) { $c.imagenes = $img }
        if ($otros.Count) { $c.archivos = $otros }
        Send-PixPin -Accion chat -Campos $c -ComoJson:$ComoJson
    }
}

function Add-PixPinPin {
    <# .SYNOPSIS Saca cada fichero a la pantalla como pin. #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta)
    process { foreach ($r in (Resolve-PixPinRuta $Ruta)) { Send-PixPin -Accion pinear -Campos @{ ruta = $r } } }
}

function Open-PixPin {
    <# .SYNOPSIS Abre un fichero en PixPin (lector, editor o pin), en una pagina o buscando un texto. #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [int]$Pagina,
        [string]$Buscar
    )
    process {
        foreach ($r in (Resolve-PixPinRuta $Ruta)) {
            $que = @{ tipo = 'fichero'; ruta = $r }
            if ($Pagina) { $que.pagina = $Pagina }
            if ($Buscar) { $que.buscar = $Buscar }
            Send-PixPin -Accion abrir -Campos @{ que = $que }
        }
    }
}

function Get-PixPinTexto {
    <#
    .SYNOPSIS
    El texto de unas imagenes (OCR de Windows), en orden de lectura. No
    necesita PixPin abierta.
    .EXAMPLE
    Get-PixPinTexto .\factura.png -Copiar
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [switch]$Copiar
    )
    process {
        $a = @('ocr') + @(Resolve-PixPinRuta $Ruta)
        if ($Copiar) { $a += '--copiar' }
        Invoke-PixPin -Argumentos $a
    }
}

function Invoke-PixPinAsistente {
    <#
    .SYNOPSIS
    Le pide algo al asistente sin abrir el chat; la respuesta sale en el aviso
    de PixPin y queda en el chat «Asistente».
    .EXAMPLE
    Invoke-PixPinAsistente 'resume mis tareas de hoy'
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)][string]$Pedido,
        [Alias('FullName')][string[]]$Archivo,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    $c = Add-PixPinCampoProyecto @{ texto = $Pedido } $Proyecto
    if ($Archivo) {
        $rutas = @(Resolve-PixPinRuta $Archivo)
        $img = @($rutas | Where-Object { $_ -match '\.(png|jpe?g|bmp|gif|webp|tif)$' })
        $otros = @($rutas | Where-Object { $_ -notmatch '\.(png|jpe?g|bmp|gif|webp|tif)$' })
        if ($img.Count) { $c.imagenes = $img }
        if ($otros.Count) { $c.texto += "`n`nArchivos:`n" + (($otros | ForEach-Object { "- $_" }) -join "`n") }
    }
    Send-PixPin -Accion asistente -Campos $c -ComoJson:$ComoJson
}

function Start-PixPinCaptura {
    <# .SYNOPSIS Empieza la captura de una zona, como el atajo general. #>
    [CmdletBinding()] param()
    Send-PixPin -Accion capturar
}

function Send-PixPinAlMovil {
    <# .SYNOPSIS Manda ficheros al movil (a todos, o a -Aparato). #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [string]$Aparato = 'todos'
    )
    begin { $todos = New-Object System.Collections.Generic.List[string] }
    process { foreach ($r in (Resolve-PixPinRuta $Ruta)) { $todos.Add($r) } }
    end {
        $imagen = '\.(png|jpe?g|bmp|gif|webp|tif)$'
        Send-PixPin -Accion enviar_al_movil -Campos @{
            aparato  = $Aparato
            imagenes = @($todos | Where-Object { $_ -match $imagen })
            archivos = @($todos | Where-Object { $_ -notmatch $imagen })
        }
    }
}

function Out-PixPin {
    <#
    .SYNOPSIS
    Convierte lo que llega por la tuberia en una tabla del chat (la del pin de
    hoja, que viaja al movil) y la abre.
    .EXAMPLE
    Import-Csv .\gastos.csv | Out-PixPin -Tabla 'Gastos de octubre' -Proyecto Obra
    #>
    [CmdletBinding()]
    param(
        [Parameter(ValueFromPipeline = $true)]$Objeto,
        [Parameter(Mandatory = $true)][string]$Tabla,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    begin { $todos = New-Object System.Collections.Generic.List[object] }
    process { if ($null -ne $Objeto) { $todos.Add($Objeto) } }
    end {
        $filas = New-Object System.Collections.Generic.List[object]
        $cabecera = $false
        if ($todos.Count) {
            $primero = $todos[0]
            if ($primero -is [string] -or $primero.GetType().IsPrimitive) {
                foreach ($o in $todos) { $filas.Add(@([string]$o)) }
            } else {
                $props = @($primero.PSObject.Properties | ForEach-Object { $_.Name })
                $filas.Add($props)
                $cabecera = $true
                foreach ($o in $todos) { $filas.Add(@($props | ForEach-Object { [string]$o.$_ })) }
            }
        }
        $c = Add-PixPinCampoProyecto @{ nombre = $Tabla; filas = $filas.ToArray(); cabecera = $cabecera; abrir = $true } $Proyecto
        Send-PixPin -Accion tabla_nueva -Campos $c -ComoJson:$ComoJson
    }
}

function Open-PixPinEnlace {
    <# .SYNOPSIS Hace un enlace pixpin:// (lo mismo que pulsarlo, con su lista blanca). #>
    [CmdletBinding()] param([Parameter(Mandatory = $true, Position = 0)][string]$Url)
    Invoke-PixPin -Argumentos @('--url', $Url)
}

Export-ModuleMember -Function Get-PixPinExe, Invoke-PixPin, Send-PixPin, New-PixPinTarea, New-PixPinNota,
    Send-PixPinChat, Add-PixPinPin, Open-PixPin, Get-PixPinTexto, Invoke-PixPinAsistente, Start-PixPinCaptura,
    Send-PixPinAlMovil, Out-PixPin, Open-PixPinEnlace
