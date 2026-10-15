# Registra el puente de la extension del navegador (apps/pixpin-puente) como
# "host" de mensajeria nativa de Chrome y de Edge, solo para este usuario
# (HKCU, sin administrador). Lo llama herramientas\instalar.ps1; a mano:
#
#   powershell -ExecutionPolicy Bypass -File herramientas\registrar-navegador.ps1 `
#       [-Puente <ruta a pixpin-puente.exe>] [-SoloMostrar] [-Quitar]
#
# Hace dos cosas:
#   1. Escribe junto al exe el manifiesto del host (pixpin-puente.json): su
#      nombre, la ruta del exe y la UNICA extension que puede hablarle (la de
#      extensiones\navegador, cuyo id es fijo porque su manifest.json lleva
#      "key").
#   2. Apunta a ese manifiesto desde
#        HKCU\Software\Google\Chrome\NativeMessagingHosts\com.pixpinmax.puente
#        HKCU\Software\Microsoft\Edge\NativeMessagingHosts\com.pixpinmax.puente
#
# -SoloMostrar: dice lo que haria y ensena el manifiesto, sin escribir nada
# (ni fichero ni registro). -Quitar: borra las dos claves y el manifiesto.

param(
    [string]$Puente = (Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax\pixpin-puente.exe'),
    [switch]$SoloMostrar,
    [switch]$Quitar
)

$ErrorActionPreference = 'Stop'

# El nombre del host (nucleo.js: HOST) y el id de la extension, que sale de
# la "key" de extensiones\navegador\manifest.json (la prueba de Node
# comprueba que coinciden).
$nombre = 'com.pixpinmax.puente'
$idExtension = 'hcbhbfnjmdofjkdgeljognmjgppdiflo'

$manifiesto = Join-Path (Split-Path -Parent $Puente) 'pixpin-puente.json'
$claves = @(
    "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$nombre",
    "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$nombre"
)

$contenido = [ordered]@{
    name            = $nombre
    description     = 'PixPin Max: puente de la extension del navegador'
    path            = $Puente
    type            = 'stdio'
    allowed_origins = @("chrome-extension://$idExtension/")
} | ConvertTo-Json -Depth 3

if ($SoloMostrar) {
    Write-Host '[solo mostrar] No se escribe nada.'
    if ($Quitar) {
        Write-Host "[solo mostrar] Borraria el fichero: $manifiesto"
        foreach ($c in $claves) { Write-Host "[solo mostrar] Borraria la clave: $c" }
        exit 0
    }
    if (-not (Test-Path $Puente)) { Write-Host "[solo mostrar] Ojo: todavia no existe $Puente" }
    Write-Host "[solo mostrar] Escribiria el manifiesto: $manifiesto"
    Write-Output $contenido
    foreach ($c in $claves) { Write-Host "[solo mostrar] Pondria en $c (predeterminado) = $manifiesto" }
    exit 0
}

if ($Quitar) {
    foreach ($c in $claves) {
        if (Test-Path $c) { Remove-Item $c -Force; Write-Host "Quitada: $c" }
    }
    if (Test-Path $manifiesto) { Remove-Item $manifiesto -Force; Write-Host "Quitado: $manifiesto" }
    exit 0
}

if (-not (Test-Path $Puente)) { throw "No existe $Puente" }
# Sin BOM: el navegador lee el manifiesto como JSON estricto.
[System.IO.File]::WriteAllText($manifiesto, $contenido, (New-Object System.Text.UTF8Encoding($false)))
foreach ($c in $claves) {
    New-Item -Path $c -Force | Out-Null
    Set-Item -Path $c -Value $manifiesto
}
Write-Host "Puente del navegador registrado para Chrome y Edge: $manifiesto"
