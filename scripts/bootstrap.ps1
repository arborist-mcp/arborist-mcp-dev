param(
    [string]$Python = "python"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path ".venv")) {
    & $Python -m venv .venv
}

. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install maturin
maturin develop
& $PSScriptRoot\sync-extension.ps1

Write-Host "Bootstrap complete. Activate with . .\\.venv\\Scripts\\Activate.ps1"
