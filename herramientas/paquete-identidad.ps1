# El paquete de IDENTIDAD de PixPin Max (MSIX con ubicacion externa): lo
# genera, lo firma con un certificado propio, hace que el equipo confie en ese
# certificado y lo registra apuntando a la carpeta instalada. Con el paquete
# registrado PixPin sale en el panel Compartir de otras apps y en el menu
# contextual de primer nivel de Windows 11. Ver docs/integraciones-windows.md.
#
# Uso (por pasos, o -Todo):
#   powershell -ExecutionPolicy Bypass -File herramientas\paquete-identidad.ps1 -SoloMostrar
#   ... -Crear       genera el .msix (sin firmar) en -Salida
#   ... -Firmar      crea el certificado "CN=PixPin Max Local" en CurrentUser\My
#                    si no esta, lo exporta a .cer y firma el .msix
#   ... -Confiar     mete el .cer en LocalMachine\TrustedPeople (PIDE ADMINISTRADOR)
#   ... -Registrar   Add-AppxPackage -ExternalLocation <carpeta instalada>
#   ... -Todo        los cuatro, en orden
#   ... -Quitar      desregistra el paquete (y con -BorrarCertificado, quita el
#                    certificado de los dos almacenes; esto PIDE ADMINISTRADOR)
#   [-Instalado <carpeta>]  por defecto %LOCALAPPDATA%\Programs\PixPinMax
#   [-Salida <carpeta>]     por defecto herramientas\paquete-identidad\salida
#
# Requisitos: Windows 10 2004 o mas nuevo; makeappx.exe y signtool.exe del SDK
# de Windows (se buscan en "Windows Kits\10\bin\<version>\x64"); y que el
# pixpinmax.exe instalado lleve en su manifiesto el elemento <msix> (desde el
# 15-oct-2026, apps/pixpin/pixpinmax.manifest): sin el, Windows registra el
# paquete pero el exe no recibe la identidad.

param(
    [switch]$SoloMostrar,
    [switch]$Crear,
    [switch]$Firmar,
    [switch]$Confiar,
    [switch]$Registrar,
    [switch]$Todo,
    [switch]$Quitar,
    [switch]$BorrarCertificado,
    [string]$Instalado,
    [string]$Salida,
    # Uso interno: la copia elevada de -Confiar / -BorrarCertificado.
    [switch]$Elevado
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$fuente = Join-Path $PSScriptRoot 'paquete-identidad'
if (-not $Instalado) { $Instalado = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax' }
if (-not $Salida) { $Salida = Join-Path $fuente 'salida' }
$sujeto = 'CN=PixPin Max Local'
$nombrePaquete = 'ForgePixPin.PixPinMax'
$msix = Join-Path $Salida 'PixPinMax-identidad.msix'
$cer = Join-Path $Salida 'PixPinMax-identidad.cer'
if ($Todo) { $Crear = $true; $Firmar = $true; $Confiar = $true; $Registrar = $true }
if (-not ($Crear -or $Firmar -or $Confiar -or $Registrar -or $Quitar)) { $SoloMostrar = $true }

function Es-Admin {
    $p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# La herramienta del SDK mas nueva (x64).
function Herramienta([string]$exe) {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (-not (Test-Path $kits)) { return $null }
    Get-ChildItem $kits -Directory |
        Where-Object { $_.Name -match '^10\.' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\$exe" } |
        Where-Object { Test-Path $_ } |
        Select-Object -First 1
}

# La version del paquete: la del workspace (Cargo.toml) con un cuarto numero 0.
function Version {
    $linea = Select-String -Path (Join-Path $repo 'Cargo.toml') -Pattern '^version\s*=\s*"([0-9.]+)"' | Select-Object -First 1
    $v = if ($linea) { $linea.Matches[0].Groups[1].Value } else { '0.1.0' }
    while (($v.Split('.')).Count -lt 4) { $v += '.0' }
    return $v
}

function Certificado {
    Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $sujeto } |
        Sort-Object NotAfter -Descending | Select-Object -First 1
}

$makeappx = Herramienta 'makeappx.exe'
$signtool = Herramienta 'signtool.exe'
$exe = Join-Path $Instalado 'pixpinmax.exe'
$cert = Certificado
$confiado = Get-ChildItem Cert:\LocalMachine\TrustedPeople -ErrorAction SilentlyContinue | Where-Object { $_.Subject -eq $sujeto }
$registrado = Get-AppxPackage -Name $nombrePaquete -ErrorAction SilentlyContinue
$conMsix = $false
if (Test-Path $exe) {
    # El manifiesto va incrustado como texto: basta buscar el elemento.
    $bytes = [IO.File]::ReadAllBytes($exe)
    $conMsix = [Text.Encoding]::ASCII.GetString($bytes).Contains('urn:schemas-microsoft-com:msix.v1')
}

Write-Host 'Paquete de identidad de PixPin Max'
Write-Host "  Windows:                 $([Environment]::OSVersion.Version) (hace falta 10.0.19041 o mas)"
Write-Host "  makeappx.exe:            $(if ($makeappx) {$makeappx} else {'NO esta (instala el SDK de Windows)'})"
Write-Host "  signtool.exe:            $(if ($signtool) {$signtool} else {'NO esta (instala el SDK de Windows)'})"
Write-Host "  pixpinmax.exe instalado: $(if (Test-Path $exe) {$exe} else {"NO esta en $Instalado"})"
Write-Host "  ...con <msix> dentro:    $(if ($conMsix) {'si'} else {'no (instala una compilacion del 15-oct-2026 o posterior)'})"
Write-Host "  Certificado propio:      $(if ($cert) {"$($cert.Thumbprint) (caduca $($cert.NotAfter.ToString('yyyy-MM-dd')))"} else {'no creado'})"
Write-Host "  Confiado en el equipo:   $(if ($confiado) {'si'} else {'no'})"
Write-Host "  Paquete registrado:      $(if ($registrado) {"si, $($registrado.PackageFullName)"} else {'no'})"
Write-Host "  Paquete generado:        $(if (Test-Path $msix) {$msix} else {'no'})"

if ($SoloMostrar) {
    Write-Host ''
    Write-Host 'Pasos (docs/integraciones-windows.md):'
    Write-Host "  1. -Crear      makeappx pack -> $msix"
    Write-Host "  2. -Firmar     New-SelfSignedCertificate '$sujeto' (CurrentUser\My) + signtool sign"
    Write-Host '  3. -Confiar    Import-Certificate -> LocalMachine\TrustedPeople  (administrador, una vez)'
    Write-Host "  4. -Registrar  Add-AppxPackage -Path <msix> -ExternalLocation '$Instalado'"
    Write-Host ''
    Write-Host '-SoloMostrar: no se ha cambiado nada.'
    return
}

# Lo que pide administrador, en una copia elevada (una vez).
function Elevar([string[]]$pasos) {
    $argumentos = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"",
        '-Salida', "`"$Salida`"", '-Instalado', "`"$Instalado`"", '-Elevado') + $pasos
    Write-Host 'Pidiendo permiso de administrador (una vez)...'
    $p = Start-Process -FilePath 'powershell.exe' -ArgumentList $argumentos -Verb RunAs -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "La parte de administrador fallo (codigo $($p.ExitCode))." }
}

if ($Quitar) {
    if ($registrado -and -not $Elevado) {
        Remove-AppxPackage -Package $registrado.PackageFullName
        Write-Host "Desregistrado: $($registrado.PackageFullName)"
    }
    if ($BorrarCertificado) {
        if (-not (Es-Admin)) { Elevar @('-Quitar', '-BorrarCertificado'); return }
        Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object { $_.Subject -eq $sujeto } | Remove-Item
        Write-Host 'Certificado quitado de LocalMachine\TrustedPeople.'
        if (-not $Elevado) {
            Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -eq $sujeto } | Remove-Item
            Write-Host 'Certificado quitado de CurrentUser\My.'
        }
    }
    return
}

if ($Crear -and -not $Elevado) {
    if (-not $makeappx) { throw 'Falta makeappx.exe: instala el SDK de Windows (ver docs/integraciones-windows.md).' }
    $obra = Join-Path $Salida 'obra'
    if (Test-Path $obra) { Remove-Item -Recurse -Force $obra }
    New-Item -ItemType Directory -Force -Path (Join-Path $obra 'Assets') | Out-Null
    # El manifiesto, con la version de hoy.
    [xml]$m = Get-Content -Raw -Encoding UTF8 (Join-Path $fuente 'AppxManifest.xml')
    $m.Package.Identity.Version = (Version)
    if ($m.Package.Identity.Publisher -ne $sujeto) { throw "El editor del manifiesto no es '$sujeto'." }
    $m.Save((Join-Path $obra 'AppxManifest.xml'))
    # Los logos, del icono de la app.
    Add-Type -AssemblyName System.Drawing
    $ico = Join-Path $repo 'apps\pixpin\recursos\pixpinmax.ico'
    # El icono mas grande del .ico. Los de 256 suelen ir en PNG, que
    # System.Drawing.Icon no lee: se saca cada imagen a mano (cabecera ICO:
    # 6 bytes y 16 por entrada) y se abre como imagen.
    $datos = [IO.File]::ReadAllBytes($ico)
    $grande = $null
    $n = [BitConverter]::ToUInt16($datos, 4)
    $mejor = -1; $lado = 0
    for ($i = 0; $i -lt $n; $i++) {
        $w = $datos[6 + 16 * $i]; if ($w -eq 0) { $w = 256 }
        if ($w -gt $lado) { $lado = $w; $mejor = $i }
    }
    if ($mejor -ge 0) {
        $largo = [BitConverter]::ToInt32($datos, 6 + 16 * $mejor + 8)
        $desde = [BitConverter]::ToInt32($datos, 6 + 16 * $mejor + 12)
        if ($datos[$desde] -eq 0x89) {
            $grande = [System.Drawing.Image]::FromStream((New-Object IO.MemoryStream($datos, $desde, $largo)))
        } else {
            $grande = (New-Object System.Drawing.Icon($ico, $lado, $lado)).ToBitmap()
        }
    }
    if (-not $grande) { throw "No se pudo leer el icono $ico." }
    foreach ($par in @(@('StoreLogo.png', 50), @('Square150x150Logo.png', 150), @('Square44x44Logo.png', 44))) {
        $b = New-Object System.Drawing.Bitmap($par[1], $par[1])
        $g = [System.Drawing.Graphics]::FromImage($b)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.DrawImage($grande, 0, 0, $par[1], $par[1])
        $g.Dispose()
        $b.Save((Join-Path $obra "Assets\$($par[0])"), [System.Drawing.Imaging.ImageFormat]::Png)
        $b.Dispose()
    }
    $grande.Dispose()
    # /nv: el paquete NO lleva pixpinmax.exe (esta en la ubicacion externa) y
    # la validacion completa lo exigiria. El esquema del manifiesto se
    # comprobo sin /nv el 15-oct-2026 (solo fallaba eso).
    & $makeappx pack /o /nv /d $obra /p $msix
    if ($LASTEXITCODE -ne 0) { throw "makeappx fallo ($LASTEXITCODE)." }
    Write-Host "Generado: $msix"
}

if ($Firmar -and -not $Elevado) {
    if (-not $signtool) { throw 'Falta signtool.exe: instala el SDK de Windows (ver docs/integraciones-windows.md).' }
    if (-not (Test-Path $msix)) { throw "Falta $msix (paso -Crear)." }
    $cert = Certificado
    if (-not $cert) {
        # Firma de codigo (EKU 1.3.6.1.5.5.7.3.3), sin ser entidad emisora.
        $cert = New-SelfSignedCertificate -Type Custom -Subject $sujeto -KeyUsage DigitalSignature `
            -FriendlyName 'PixPin Max (paquete de identidad)' -CertStoreLocation 'Cert:\CurrentUser\My' `
            -NotAfter (Get-Date).AddYears(10) `
            -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
        Write-Host "Certificado creado: $($cert.Thumbprint)"
    }
    Export-Certificate -Cert $cert -FilePath $cer | Out-Null
    & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint /s My $msix
    if ($LASTEXITCODE -ne 0) { throw "signtool fallo ($LASTEXITCODE)." }
    Write-Host "Firmado: $msix  (certificado publico: $cer)"
}

if ($Confiar) {
    if (-not (Test-Path $cer)) { throw "Falta $cer (paso -Firmar)." }
    if (-not (Es-Admin)) {
        Elevar @('-Confiar')
    } else {
        Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null
        Write-Host 'El equipo confia en el certificado (LocalMachine\TrustedPeople).'
        if ($Elevado) { return }
    }
}

if ($Registrar -and -not $Elevado) {
    if (-not (Test-Path $msix)) { throw "Falta $msix (pasos -Crear y -Firmar)." }
    if (-not (Test-Path $exe)) { throw "No esta $exe (instala PixPin con herramientas\instalar.ps1)." }
    if (-not $conMsix) { Write-Warning 'El pixpinmax.exe instalado no lleva <msix>: el paquete se registra pero no le dara identidad.' }
    Add-AppxPackage -Path $msix -ExternalLocation $Instalado -ForceUpdateFromAnyVersion
    $r = Get-AppxPackage -Name $nombrePaquete
    Write-Host "Registrado: $($r.PackageFullName)"
    Write-Host "AUMID del paquete: $($r.PackageFamilyName)!PixPinMax"
    Write-Host 'Cierra y abre PixPin: desde el acceso directo de siempre ya arranca con identidad.'
}
