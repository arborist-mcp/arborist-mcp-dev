param(
    [string]$Python = "python"
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$venvDir = Join-Path $repoRoot ".venv"
$activateScript = Join-Path $venvDir "Scripts\Activate.ps1"

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

Push-Location $repoRoot
try {
    if (-not (Test-Path $venvDir)) {
        Invoke-NativeOrThrow "Creating virtual environment" $Python @("-m", "venv", $venvDir)
    }

    . $activateScript
    Invoke-NativeOrThrow "Upgrading pip" "python" @("-m", "pip", "install", "--upgrade", "pip")
    Invoke-NativeOrThrow "Installing maturin" "python" @("-m", "pip", "install", "maturin")
    Invoke-NativeOrThrow "Building extension with maturin" "maturin" @("develop", "--locked")
    & $PSScriptRoot\sync-extension.ps1 -SkipBuild
} finally {
    Pop-Location
}

Write-Host "Bootstrap complete. Activate with . .\\.venv\\Scripts\\Activate.ps1"
