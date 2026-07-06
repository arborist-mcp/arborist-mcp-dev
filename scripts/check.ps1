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

    Write-Host $Description
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

Push-Location $repoRoot
try {
    Invoke-NativeOrThrow "Running Rust tests..." "cargo" @("test")
    Invoke-NativeOrThrow "Running Rust clippy..." "cargo" @("clippy", "--all-targets", "--", "-D", "warnings")
    Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest")
    Invoke-NativeOrThrow "Checking gateway CLI..." $Python @("-m", "arborist_mcp.gateway", "--help")
} finally {
    Pop-Location
}

Write-Host "All checks passed."
