param()

$ErrorActionPreference = "Stop"

$source = Join-Path $PSScriptRoot "..\target\debug\_arborist_core.dll"
$destination = Join-Path $PSScriptRoot "..\python\arborist_mcp\_arborist_core.pyd"

if (-not (Test-Path $source)) {
    throw "Compiled extension not found at $source. Run `maturin develop` first."
}

Copy-Item $source $destination -Force
Write-Host "Synced extension to $destination"
