[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$Version,
  [Parameter(Mandatory=$true)][string]$Platform
)

$ErrorActionPreference = 'Stop'

function Assert-FileExists {
  param([Parameter(Mandatory=$true)][string]$Path)

  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Missing required file: $Path"
  }
}

function Copy-RequiredFile {
  param(
    [Parameter(Mandatory=$true)][string]$Source,
    [Parameter(Mandatory=$true)][string]$Destination
  )

  Assert-FileExists -Path $Source

  $destinationDir = Split-Path -Parent $Destination
  if ($destinationDir) {
    New-Item -ItemType Directory -Force -Path $destinationDir | Out-Null
  }

  Copy-Item -Force $Source $Destination
}

$serverDir = "men-among-gods-server-$Version-$Platform"
$clientDir = "men-among-gods-client-$Version-$Platform"
$serverPackageDir = Join-Path 'dist' $serverDir
$clientPackageDir = Join-Path 'dist' $clientDir

Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $serverPackageDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $clientPackageDir 'assets') | Out-Null

Copy-RequiredFile -Source 'target/release/server.exe' -Destination (Join-Path $serverPackageDir 'server.exe')
Copy-RequiredFile -Source 'target/release/map_viewer.exe' -Destination (Join-Path $serverPackageDir 'map_viewer.exe')
Copy-RequiredFile -Source 'target/release/template_viewer.exe' -Destination (Join-Path $serverPackageDir 'template_viewer.exe')
Copy-RequiredFile -Source 'target/release/api.exe' -Destination (Join-Path $serverPackageDir 'api.exe')
Copy-RequiredFile -Source 'target/release/world-snapshot.exe' -Destination (Join-Path $serverPackageDir 'world-snapshot.exe')
Copy-RequiredFile -Source 'server/assets/world_seed.wsnap' -Destination (Join-Path $serverPackageDir 'assets/world_seed.wsnap')

Copy-Item -Recurse -Force 'client/assets/*' (Join-Path $clientPackageDir 'assets/')
Copy-RequiredFile -Source 'target/release/men-among-gods-client.exe' -Destination (Join-Path $clientPackageDir 'men-among-gods-client.exe')

Compress-Archive -Force -Path $serverPackageDir -DestinationPath "dist/$serverDir.zip"
Compress-Archive -Force -Path $clientPackageDir -DestinationPath "dist/$clientDir.zip"
