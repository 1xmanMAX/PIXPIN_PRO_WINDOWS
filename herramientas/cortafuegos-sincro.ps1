# Abre en el cortafuegos de Windows los puertos de Sincronizar de PixPin Max.
# Uso (como administrador):
#   powershell -ExecutionPolicy Bypass -File herramientas\cortafuegos-sincro.ps1
#              [-Programa <ruta a pixpinmax.exe>] [-Quitar]
#
# Normalmente NO hace falta: la primera vez que PixPin escucha (TCP 47474),
# Windows pregunta «¿Permitir?» y la regla que crea al pulsar «Permitir» es
# para el PROGRAMA, con TCP y UDP, asi que tambien vale para la baliza de
# «Descubrir rápido» (UDP 47475, docs/movil-como-camara.md). Este script es
# para cuando no se pregunto (lo bloqueo una directiva de empresa, se pulso
# «Cancelar») o la red esta marcada como «Pública», donde esa regla no vale:
# pone reglas para el programa solo en el perfil «Privado» y «Dominio».
#
# Mejor que abrir la red «Pública»: marcar la Wi-Fi de casa o de clase como
# «Privada» (Configuracion > Red e Internet > Wi-Fi > la red > Tipo de perfil).

param([string]$Programa, [switch]$Quitar)

$ErrorActionPreference = 'Stop'
$nombre = 'PixPin Max - Sincronizar'

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { throw 'Hace falta abrir PowerShell como administrador.' }

Get-NetFirewallRule -DisplayName "$nombre*" -ErrorAction SilentlyContinue | Remove-NetFirewallRule
if ($Quitar) { Write-Host 'Reglas quitadas.'; return }

if (-not $Programa) { $Programa = Join-Path $env:LOCALAPPDATA 'Programs\PixPinMax\pixpinmax.exe' }
if (-not (Test-Path $Programa)) { throw "No esta $Programa (pasa -Programa)" }

# TCP 47474: la sincronizacion (o el puerto que diga «La de este aparato»,
# si estaba ocupado: por eso va por programa y no solo por puerto).
New-NetFirewallRule -DisplayName "$nombre (TCP)" -Direction Inbound -Action Allow `
    -Program $Programa -Protocol TCP -Profile Private,Domain | Out-Null
# UDP 47475: la baliza (difusion, multidifusion 239.255.74.74 y directa).
New-NetFirewallRule -DisplayName "$nombre (baliza UDP 47475)" -Direction Inbound -Action Allow `
    -Program $Programa -Protocol UDP -LocalPort 47475 -Profile Private,Domain | Out-Null
Write-Host "Listo: TCP y UDP 47475 abiertos para $Programa en redes privadas y de dominio."
