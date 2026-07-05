param()

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$source = Join-Path $repoRoot "target\debug\_arborist_core.dll"
$destination = Join-Path $repoRoot "python\arborist_mcp\_arborist_core.pyd"

Push-Location $repoRoot
try {
    Write-Host "Building arborist-py debug extension..."
    & cargo build -p arborist-py
} finally {
    Pop-Location
}

Copy-Item $source $destination -Force
Write-Host "Synced extension to $destination"
