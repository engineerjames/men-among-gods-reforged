[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)][string]$Version,
  [Parameter(Mandatory=$true)][string]$Platform
)

$ErrorActionPreference = 'Stop'

$serverDir = "men-among-gods-server-$Version-$Platform"
$clientDir = "men-among-gods-client-$Version-$Platform"

Remove-Item -Recurse -Force dist -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path "dist/$serverDir/.dat" | Out-Null
New-Item -ItemType Directory -Force -Path "dist/$clientDir/assets" | Out-Null

Copy-Item -Recurse -Force "server/assets/.dat/*" "dist/$serverDir/.dat/"
Copy-Item -Force "target/release/server.exe" "dist/$serverDir/server.exe"

Copy-Item -Recurse -Force "client/assets/*" "dist/$clientDir/assets/"
Copy-Item -Force "target/release/client.exe" "dist/$clientDir/client.exe"

Compress-Archive -Force -Path "dist/$serverDir" -DestinationPath "dist/$serverDir.zip"
Compress-Archive -Force -Path "dist/$clientDir" -DestinationPath "dist/$clientDir.zip"
