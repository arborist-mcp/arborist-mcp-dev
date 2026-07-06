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

function Invoke-GatewayInitializeSmoke {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python
    )

    Write-Host "Checking gateway initialize..."
    $request = New-TemporaryFile
    try {
        Set-Content -LiteralPath $request -Encoding UTF8 -Value '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
        $output = & $Python @("-m", "arborist_mcp.gateway", "--once", $request) 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "Checking gateway initialize failed with exit code $LASTEXITCODE.`n$output"
        }

        $response = $output | Out-String | ConvertFrom-Json
        if ($response.PSObject.Properties.Name -contains "error") {
            $message = $response.error.message
            throw "Checking gateway initialize returned JSON-RPC error: $message"
        }
        if (-not ($response.result.supportedLanguages -contains "python")) {
            throw "Checking gateway initialize did not report Python support."
        }
        if (-not ($response.result.supportedLanguages -contains "c")) {
            throw "Checking gateway initialize did not report C support."
        }
    } finally {
        Remove-Item -LiteralPath $request -Force -ErrorAction SilentlyContinue
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

Push-Location $repoRoot
try {
    Invoke-NativeOrThrow "Checking Rust formatting..." "cargo" @("fmt", "--check")
    Invoke-NativeOrThrow "Running Rust tests..." "cargo" @("test")
    Invoke-NativeOrThrow "Running Rust clippy..." "cargo" @("clippy", "--all-targets", "--", "-D", "warnings")
    Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest")
    Invoke-NativeOrThrow "Checking gateway CLI..." $Python @("-m", "arborist_mcp.gateway", "--help")
    Invoke-GatewayInitializeSmoke $Python
} finally {
    Pop-Location
}

Write-Host "All checks passed."
