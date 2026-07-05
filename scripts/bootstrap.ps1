param(
    [string]$Python = "python"
)

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

if (-not (Test-Path ".venv")) {
    Invoke-NativeOrThrow "Creating virtual environment" $Python @("-m", "venv", ".venv")
}

. .\.venv\Scripts\Activate.ps1
Invoke-NativeOrThrow "Upgrading pip" "python" @("-m", "pip", "install", "--upgrade", "pip")
Invoke-NativeOrThrow "Installing maturin" "python" @("-m", "pip", "install", "maturin")
Invoke-NativeOrThrow "Building extension with maturin" "maturin" @("develop")
& $PSScriptRoot\sync-extension.ps1

Write-Host "Bootstrap complete. Activate with . .\\.venv\\Scripts\\Activate.ps1"
