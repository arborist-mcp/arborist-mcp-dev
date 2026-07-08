param(
    [string]$Python = "python",
    [switch]$Quiet,
    [ValidateSet(
        "rust",
        "python",
        "gateway",
        "gateway-request-validation",
        "gateway-symbol-routes",
        "gateway-execution",
        "gateway-trace-payloads",
        "gateway-runtime",
        "inner-loop",
        "all"
    )]
    [string]$Suite = "inner-loop"
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

    if (-not $Quiet) {
        Write-Host $Description
    }
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Invoke-GatewaySuite {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$ModuleName
    )

    Invoke-NativeOrThrow "Running $ModuleName..." $Python @("-m", "unittest", $ModuleName)
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

$gatewaySuites = @{
    "gateway-request-validation" = "tests.gateway_protocol.request_validation"
    "gateway-symbol-routes" = "tests.gateway_protocol.symbol_routes"
    "gateway-execution" = "tests.gateway_protocol.execution"
    "gateway-trace-payloads" = "tests.gateway_protocol.trace_payloads"
    "gateway-runtime" = "tests.gateway_protocol.runtime"
}

Push-Location $repoRoot
try {
    switch ($Suite) {
        "rust" {
            Invoke-NativeOrThrow "Running Rust tests..." "cargo" @("test", "--locked")
        }
        "python" {
            Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest", "discover", "-s", "tests")
        }
        "gateway" {
            Invoke-GatewaySuite $Python "tests.test_gateway_protocol"
        }
        "gateway-request-validation" {
            Invoke-GatewaySuite $Python $gatewaySuites[$Suite]
        }
        "gateway-symbol-routes" {
            Invoke-GatewaySuite $Python $gatewaySuites[$Suite]
        }
        "gateway-execution" {
            Invoke-GatewaySuite $Python $gatewaySuites[$Suite]
        }
        "gateway-trace-payloads" {
            Invoke-GatewaySuite $Python $gatewaySuites[$Suite]
        }
        "gateway-runtime" {
            Invoke-GatewaySuite $Python $gatewaySuites[$Suite]
        }
        "inner-loop" {
            Invoke-NativeOrThrow "Running Rust tests..." "cargo" @("test", "--locked")
            Invoke-GatewaySuite $Python "tests.test_gateway_protocol"
        }
        "all" {
            Invoke-NativeOrThrow "Running Rust tests..." "cargo" @("test", "--locked")
            Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest", "discover", "-s", "tests")
        }
    }
} finally {
    Pop-Location
}

if (-not $Quiet) {
    Write-Host "Requested test suite passed."
}
