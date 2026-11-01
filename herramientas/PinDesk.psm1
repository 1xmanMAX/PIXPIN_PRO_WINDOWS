# PinDesk.psm1 — pedirle cosas a PinDesk desde PowerShell.
#
#   Import-Module "$env:LOCALAPPDATA\Programs\PinDesk\PinDesk.psm1"
#   New-PinDeskTarea "llamar al ingeniero" -Proyecto Obra
#   Get-ChildItem *.png | Send-PinDeskChat -Texto "fotos de hoy"
#   Get-PinDeskTexto .\captura.png | Set-Clipboard
#   Get-Process | Select-Object Name, CPU -First 10 | Out-PinDesk -Tabla "Procesos"
#
# Todo va por la linea de ordenes de pindesk.exe (docs/integraciones.md):
# este modulo no habla con la app por su cuenta. Los pedidos se le pasan por
# la entrada estandar (`--pedido-archivo -`), asi un texto con comillas,
# saltos de linea o acentos llega entero sin pelearse con las reglas de
# comillas de Windows.
#
# Cada funcion devuelve lo que la orden escribe (el texto del OCR, la ayuda)
# y deja el codigo de salida en $LASTEXITCODE: 0 aceptado, 1 fallo local,
# 2 version, 3 no se entiende, 4 PinDesk no esta abierta, 5 no contesto,
# 6 rechazado. Si no es 0, ademas escribe un error (Write-Error).
#
# El ejecutable: $env:PINDESK_EXE (o el $env:PIXPIN_EXE de antes), o el que esta junto a este modulo, o el
# instalado en %LOCALAPPDATA%\Programs\PinDesk (o, si aun no se actualizo,
# el pixpinmax.exe de cuando se llamaba PixPin Max).

Set-StrictMode -Version 2

function Get-PinDeskExe {
    <# .SYNOPSIS Donde esta pindesk.exe. #>
    foreach ($v in $env:PINDESK_EXE, $env:PIXPIN_EXE) { if ($v -and (Test-Path $v)) { return $v } }
    $sitios = @(
        (Join-Path $PSScriptRoot 'pindesk.exe'),
        (Join-Path $PSScriptRoot 'pixpinmax.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\PinDesk\pindesk.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax\pixpinmax.exe')
    )
    foreach ($s in $sitios) { if (Test-Path $s) { return $s } }
    throw 'No se encuentra pindesk.exe (pon su ruta en $env:PINDESK_EXE)'
}

function Invoke-PinDesk {
    <#
    .SYNOPSIS
    Corre pindesk.exe con esos argumentos y espera a que acabe.
    .DESCRIPTION
    pindesk.exe es una aplicacion de ventanas: lanzada a secas, la consola no
    la espera ni ve lo que escribe. Aqui se lanza con su salida redirigida y se
    espera, asi el texto y el codigo de salida llegan.
    .EXAMPLE
    Invoke-PinDesk capturar
    .EXAMPLE
    Invoke-PinDesk ayuda
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0, ValueFromRemainingArguments = $true)][string[]]$Argumentos,
        # Lo que se le da por la entrada estandar (un pedido JSON).
        [string]$Entrada
    )
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = Get-PinDeskExe
    $info.Arguments = (@($Argumentos) | Where-Object { $_ -ne $null } | ForEach-Object { ConvertTo-PinDeskArgumento $_ }) -join ' '
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
        $motivo = ($errores.Trim(), "pindesk salio con $($p.ExitCode)" | Where-Object { $_ } | Select-Object -First 1)
        Write-Error -Message $motivo -Category InvalidOperation
    }
    if ($texto) { $texto.TrimEnd("`r", "`n") }
}

function ConvertTo-PinDeskArgumento([string]$a) {
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

function Send-PinDesk {
    <#
    .SYNOPSIS
    Manda un pedido del protocolo (docs/protocolo-pedidos.md) a PinDesk.
    .EXAMPLE
    Send-PinDesk -Accion anadir_tarea -Campos @{ texto = 'pan'; proyecto = 'pr-1' }
    .EXAMPLE
    Send-PinDesk -Json '{"pindesk":1,"accion":"capturar"}'
    #>
    [CmdletBinding(DefaultParameterSetName = 'Accion')]
    param(
        [Parameter(ParameterSetName = 'Accion', Mandatory = $true, Position = 0)][string]$Accion,
        [Parameter(ParameterSetName = 'Accion', Position = 1)][hashtable]$Campos = @{},
        [Parameter(ParameterSetName = 'Json', Mandatory = $true)][string]$Json,
        # Solo devolver el JSON, sin mandarlo.
        [switch]$ComoJson,
        # Si PinDesk no esta abierta, no abrirla (sale con 4).
        [switch]$SinArrancar
    )
    if ($PSCmdlet.ParameterSetName -eq 'Accion') {
        $pedido = [ordered]@{ pindesk = 1; accion = $Accion }
        foreach ($k in $Campos.Keys) { $pedido[$k] = $Campos[$k] }
        $Json = $pedido | ConvertTo-Json -Depth 10 -Compress
    }
    if ($ComoJson) { return $Json }
    $a = @('--pedido-archivo', '-')
    if ($SinArrancar) { $a += '--sin-arrancar' }
    Invoke-PinDesk -Argumentos $a -Entrada $Json
}

function Resolve-PinDeskRuta([string[]]$Ruta) {
    foreach ($r in $Ruta) { (Resolve-Path -LiteralPath $r -ErrorAction Stop).ProviderPath }
}

function Add-PinDeskCampoProyecto([hashtable]$c, [string]$Proyecto) {
    # Un nombre lo cambia por su id la propia orden.
    if ($Proyecto) { $c.proyecto = $Proyecto }
    $c
}

function New-PinDeskTarea {
    <#
    .SYNOPSIS
    Apunta una tarea (sin proyecto ni lista, en el Inbox). Acepta la tuberia:
    cada linea, una tarea.
    .EXAMPLE
    'pan', 'leche' | New-PinDeskTarea -Proyecto Casa
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
        $c = Add-PinDeskCampoProyecto @{ texto = $Texto } $Proyecto
        if ($Lista) { $c.codigo = $Lista }
        Send-PinDesk -Accion anadir_tarea -Campos $c -ComoJson:$ComoJson
    }
}

function New-PinDeskNota {
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
        $c = Add-PinDeskCampoProyecto @{} $Proyecto
        if ($lineas.Count) { $c.texto = $lineas -join "`n" }
        Send-PinDesk -Accion nota_nueva -Campos $c -ComoJson:$ComoJson
    }
}

function Send-PinDeskChat {
    <#
    .SYNOPSIS
    Escribe en el chat (sin proyecto, en Mensajes guardados) un texto y
    ficheros. Los ficheros pueden llegar por la tuberia (Get-ChildItem).
    .EXAMPLE
    Get-ChildItem .\fotos\*.jpg | Send-PinDeskChat -Texto 'Avance de obra' -Proyecto Obra
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)][string]$Texto = '',
        [Parameter(ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Archivo,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    begin { $todos = New-Object System.Collections.Generic.List[string] }
    process { if ($Archivo) { foreach ($r in (Resolve-PinDeskRuta $Archivo)) { $todos.Add($r) } } }
    end {
        $imagen = '\.(png|jpe?g|bmp|gif|webp|tif)$'
        $c = Add-PinDeskCampoProyecto @{ texto = $Texto } $Proyecto
        $img = @($todos | Where-Object { $_ -match $imagen })
        $otros = @($todos | Where-Object { $_ -notmatch $imagen })
        if ($img.Count) { $c.imagenes = $img }
        if ($otros.Count) { $c.archivos = $otros }
        Send-PinDesk -Accion chat -Campos $c -ComoJson:$ComoJson
    }
}

function Add-PinDeskPin {
    <# .SYNOPSIS Saca cada fichero a la pantalla como pin. #>
    [CmdletBinding()]
    param([Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta)
    process { foreach ($r in (Resolve-PinDeskRuta $Ruta)) { Send-PinDesk -Accion pinear -Campos @{ ruta = $r } } }
}

function Open-PinDesk {
    <# .SYNOPSIS Abre un fichero en PinDesk (lector, editor o pin), en una pagina o buscando un texto. #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [int]$Pagina,
        [string]$Buscar
    )
    process {
        foreach ($r in (Resolve-PinDeskRuta $Ruta)) {
            $que = @{ tipo = 'fichero'; ruta = $r }
            if ($Pagina) { $que.pagina = $Pagina }
            if ($Buscar) { $que.buscar = $Buscar }
            Send-PinDesk -Accion abrir -Campos @{ que = $que }
        }
    }
}

function Get-PinDeskTexto {
    <#
    .SYNOPSIS
    El texto de unas imagenes (OCR de Windows), en orden de lectura. No
    necesita PinDesk abierta.
    .EXAMPLE
    Get-PinDeskTexto .\factura.png -Copiar
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [switch]$Copiar
    )
    process {
        $a = @('ocr') + @(Resolve-PinDeskRuta $Ruta)
        if ($Copiar) { $a += '--copiar' }
        Invoke-PinDesk -Argumentos $a
    }
}

function Invoke-PinDeskAsistente {
    <#
    .SYNOPSIS
    Le pide algo al asistente sin abrir el chat; la respuesta sale en el aviso
    de PinDesk y queda en el chat «Asistente».
    .EXAMPLE
    Invoke-PinDeskAsistente 'resume mis tareas de hoy'
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)][string]$Pedido,
        [Alias('FullName')][string[]]$Archivo,
        [string]$Proyecto,
        [switch]$ComoJson
    )
    $c = Add-PinDeskCampoProyecto @{ texto = $Pedido } $Proyecto
    if ($Archivo) {
        $rutas = @(Resolve-PinDeskRuta $Archivo)
        $img = @($rutas | Where-Object { $_ -match '\.(png|jpe?g|bmp|gif|webp|tif)$' })
        $otros = @($rutas | Where-Object { $_ -notmatch '\.(png|jpe?g|bmp|gif|webp|tif)$' })
        if ($img.Count) { $c.imagenes = $img }
        if ($otros.Count) { $c.texto += "`n`nArchivos:`n" + (($otros | ForEach-Object { "- $_" }) -join "`n") }
    }
    Send-PinDesk -Accion asistente -Campos $c -ComoJson:$ComoJson
}

function Start-PinDeskCaptura {
    <# .SYNOPSIS Empieza la captura de una zona, como el atajo general. #>
    [CmdletBinding()] param()
    Send-PinDesk -Accion capturar
}

function Send-PinDeskAlMovil {
    <# .SYNOPSIS Manda ficheros al movil (a todos, o a -Aparato). #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true, ValueFromPipelineByPropertyName = $true)][Alias('FullName')][string[]]$Ruta,
        [string]$Aparato = 'todos'
    )
    begin { $todos = New-Object System.Collections.Generic.List[string] }
    process { foreach ($r in (Resolve-PinDeskRuta $Ruta)) { $todos.Add($r) } }
    end {
        $imagen = '\.(png|jpe?g|bmp|gif|webp|tif)$'
        Send-PinDesk -Accion enviar_al_movil -Campos @{
            aparato  = $Aparato
            imagenes = @($todos | Where-Object { $_ -match $imagen })
            archivos = @($todos | Where-Object { $_ -notmatch $imagen })
        }
    }
}

function Out-PinDesk {
    <#
    .SYNOPSIS
    Convierte lo que llega por la tuberia en una tabla del chat (la del pin de
    hoja, que viaja al movil) y la abre.
    .EXAMPLE
    Import-Csv .\gastos.csv | Out-PinDesk -Tabla 'Gastos de octubre' -Proyecto Obra
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
        $c = Add-PinDeskCampoProyecto @{ nombre = $Tabla; filas = $filas.ToArray(); cabecera = $cabecera; abrir = $true } $Proyecto
        Send-PinDesk -Accion tabla_nueva -Campos $c -ComoJson:$ComoJson
    }
}

function Open-PinDeskEnlace {
    <# .SYNOPSIS Hace un enlace pindesk:// (lo mismo que pulsarlo, con su lista blanca). #>
    [CmdletBinding()] param([Parameter(Mandatory = $true, Position = 0)][string]$Url)
    Invoke-PinDesk -Argumentos @('--url', $Url)
}

Export-ModuleMember -Function Get-PinDeskExe, Invoke-PinDesk, Send-PinDesk, New-PinDeskTarea, New-PinDeskNota,
    Send-PinDeskChat, Add-PinDeskPin, Open-PinDesk, Get-PinDeskTexto, Invoke-PinDeskAsistente, Start-PinDeskCaptura,
    Send-PinDeskAlMovil, Out-PinDesk, Open-PinDeskEnlace

# Los nombres de antes de PinDesk (`New-PixPinTarea`…), para que los perfiles
# y scripts que ya los usan sigan andando. Se pueden quitar cuando nadie los
# use (docs/formato-pindesk.md).
Set-Alias -Name Add-PixPinCampoProyecto -Value Add-PinDeskCampoProyecto
Set-Alias -Name Add-PixPinPin -Value Add-PinDeskPin
Set-Alias -Name Get-PixPinExe -Value Get-PinDeskExe
Set-Alias -Name Get-PixPinTexto -Value Get-PinDeskTexto
Set-Alias -Name Invoke-PixPin -Value Invoke-PinDesk
Set-Alias -Name Invoke-PixPinAsistente -Value Invoke-PinDeskAsistente
Set-Alias -Name New-PixPinNota -Value New-PinDeskNota
Set-Alias -Name New-PixPinTarea -Value New-PinDeskTarea
Set-Alias -Name Open-PixPin -Value Open-PinDesk
Set-Alias -Name Open-PixPinEnlace -Value Open-PinDeskEnlace
Set-Alias -Name Out-PixPin -Value Out-PinDesk
Set-Alias -Name Resolve-PixPinRuta -Value Resolve-PinDeskRuta
Set-Alias -Name Send-PixPin -Value Send-PinDesk
Set-Alias -Name Send-PixPinAlMovil -Value Send-PinDeskAlMovil
Set-Alias -Name Send-PixPinChat -Value Send-PinDeskChat
Set-Alias -Name Start-PixPinCaptura -Value Start-PinDeskCaptura
Export-ModuleMember -Alias Add-PixPinCampoProyecto, Add-PixPinPin, Get-PixPinExe, Get-PixPinTexto, Invoke-PixPin, Invoke-PixPinAsistente, New-PixPinNota, New-PixPinTarea, Open-PixPin, Open-PixPinEnlace, Out-PixPin, Resolve-PixPinRuta, Send-PixPin, Send-PixPinAlMovil, Send-PixPinChat, Start-PixPinCaptura
