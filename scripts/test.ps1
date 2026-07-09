param(
    [string]$Python = "python",
    [string[]]$Suite = @("inner-loop"),
    [string]$RustFilter,
    [switch]$Quiet,
    [switch]$ListSuites
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

function Get-GatewaySuites {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $manifestPath = Join-Path $RepoRoot "tests\gateway_protocol\suites.json"
    $rawManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $gatewaySuites = [ordered]@{}

    foreach ($property in $rawManifest.PSObject.Properties) {
        if ([string]::IsNullOrWhiteSpace($property.Name)) {
            throw "Gateway suite manifest contains a blank suite name."
        }
        if (-not ($property.Value -is [string]) -or [string]::IsNullOrWhiteSpace($property.Value)) {
            throw "Gateway suite manifest entry '$($property.Name)' must be a non-empty module name."
        }
        $gatewaySuites[$property.Name] = $property.Value
    }

    return $gatewaySuites
}

function Invoke-RustTests {
    param(
        [AllowEmptyString()]
        [string]$RustFilter
    )

    $arguments = @("test", "--locked")
    if (-not [string]::IsNullOrWhiteSpace($RustFilter)) {
        $arguments += $RustFilter
    }

    $description = "Running Rust tests..."
    if (-not [string]::IsNullOrWhiteSpace($RustFilter)) {
        $description = "Running Rust tests (filter: $RustFilter)..."
    }

    Invoke-NativeOrThrow $description "cargo" $arguments
}

function Invoke-GatewayModules {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string[]]$ModuleNames,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    Invoke-NativeOrThrow $Description $Python (@("-m", "unittest") + $ModuleNames)
}

function Invoke-PythonDiscovery {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python
    )

    Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest", "discover", "-s", "tests")
}

function Get-SuiteDescriptions {
    param(
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$GatewaySuites
    )

    $descriptions = [ordered]@{
        "rust" = "Run all Rust tests via cargo test --locked."
        "python" = "Run full Python unittest discovery under tests/."
        "gateway" = "Run the full gateway protocol meta-suite."
        "inner-loop" = "Run Rust tests plus the full gateway protocol meta-suite."
        "all" = "Run Rust tests plus the full Python unittest discovery suite."
    }

    foreach ($suiteName in $GatewaySuites.Keys) {
        $descriptions[$suiteName] = "Run only $($GatewaySuites[$suiteName])."
    }

    return $descriptions
}

function Invoke-NamedSuite {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SuiteName,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$GatewaySuites,
        [AllowEmptyString()]
        [string]$RustFilter
    )

    if ($SuiteName -eq "rust") {
        Invoke-RustTests $RustFilter
        return
    }

    if ($SuiteName -eq "python") {
        Invoke-PythonDiscovery $Python
        return
    }

    if ($SuiteName -eq "gateway") {
        Invoke-GatewayModules $Python ([string[]]$GatewaySuites.Values) "Running gateway protocol suite..."
        return
    }

    if ($GatewaySuites.Contains($SuiteName)) {
        Invoke-GatewayModules $Python @($GatewaySuites[$SuiteName]) "Running $($GatewaySuites[$SuiteName])..."
        return
    }

    if ($SuiteName -eq "inner-loop") {
        Invoke-RustTests $RustFilter
        Invoke-GatewayModules $Python ([string[]]$GatewaySuites.Values) "Running gateway protocol suite..."
        return
    }

    if ($SuiteName -eq "all") {
        Invoke-RustTests $RustFilter
        Invoke-PythonDiscovery $Python
        return
    }

    throw "Unknown suite: $SuiteName"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$gatewaySuites = Get-GatewaySuites $repoRoot
$suiteDescriptions = Get-SuiteDescriptions $gatewaySuites

if ($ListSuites) {
    foreach ($suiteName in $suiteDescriptions.Keys) {
        Write-Host ("{0,-32} {1}" -f $suiteName, $suiteDescriptions[$suiteName])
    }
    return
}

$unknownSuites = @(
    $Suite |
        Where-Object { -not $suiteDescriptions.Contains($_) } |
        Select-Object -Unique
)
if ($unknownSuites.Count -gt 0) {
    throw "Unknown suite name(s): $($unknownSuites -join ', '). Use -ListSuites to inspect supported values."
}

Push-Location $repoRoot
try {
    foreach ($suiteName in $Suite) {
        Invoke-NamedSuite $suiteName $Python $gatewaySuites $RustFilter
    }
} finally {
    Pop-Location
}

if (-not $Quiet) {
    Write-Host "Requested test suite passed."
}
