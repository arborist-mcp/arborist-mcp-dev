param()

$ErrorActionPreference = "Stop"

function Invoke-NativeOrThrow {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Description,
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [string[]]$Arguments = @()
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$source = Join-Path $repoRoot "target\debug\_arborist_core.dll"
$destination = Join-Path $repoRoot "python\arborist_mcp\_arborist_core.pyd"

Push-Location $repoRoot
try {
    Write-Host "Building arborist-py debug extension..."
    Invoke-NativeOrThrow "Building arborist-py debug extension" "cargo" @("build", "-p", "arborist-py")
} finally {
    Pop-Location
}

if (-not (Test-Path $source)) {
    throw "Compiled extension not found at $source after cargo build."
}

Copy-Item $source $destination -Force
Write-Host "Synced extension to $destination"
