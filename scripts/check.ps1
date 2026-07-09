param(
    [string]$Python = "python",
    [string[]]$Profile = @("full"),
    [switch]$ListProfiles
)

$ErrorActionPreference = "Stop"
$script:GatewayExtensionPrepared = $false

function Get-CheckProfileSnapshot {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $manifestEmitter = Join-Path $RepoRoot "scripts\check_profile_manifest.py"
    $rawOutput = & $Python $manifestEmitter 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Check profile manifest helper failed with exit code $LASTEXITCODE.`n$($rawOutput | Out-String)"
    }

    $snapshot = $rawOutput | Out-String | ConvertFrom-Json
    if ($null -eq $snapshot -or $null -eq $snapshot.profiles -or $null -eq $snapshot.profile_order) {
        throw "Check profile manifest helper must define 'profiles' and 'profile_order'."
    }

    return $snapshot
}

function Get-CheckProfileMetadata {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ProfileSnapshot,
        [Parameter(Mandatory = $true)]
        [string]$ProfileName
    )

    $property = $ProfileSnapshot.profiles.PSObject.Properties[$ProfileName]
    if ($null -eq $property) {
        throw "Unknown check profile: $ProfileName"
    }

    return $property.Value
}

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

function Invoke-ScriptOrThrow {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Description,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Script
    )

    Write-Host $Description
    & $Script
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

function Invoke-PowerShellSyntaxCheck {
    Write-Host "Checking PowerShell script syntax..."
    Get-ChildItem -LiteralPath $PSScriptRoot -Filter "*.ps1" | ForEach-Object {
        $script = $_.FullName
        try {
            [scriptblock]::Create((Get-Content -LiteralPath $script -Raw)) | Out-Null
        } catch {
            throw "PowerShell syntax check failed for $script`: $($_.Exception.Message)"
        }
    }
}

function Get-RequiredRegexValue {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Pattern,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    $content = Get-Content -LiteralPath $Path -Raw
    $match = [regex]::Match($content, $Pattern)
    if (-not $match.Success) {
        throw "Could not read $Description from $Path."
    }

    return $match.Groups[1].Value
}

function Get-CargoLockPackageVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$PackageName
    )

    $content = Get-Content -LiteralPath $Path -Raw
    $escapedPackageName = [regex]::Escape($PackageName)
    $pattern = '(?ms)^\[\[package\]\]\s*(?:(?!^\[\[package\]\]).)*?^name\s*=\s*"' + $escapedPackageName + '"\s*(?:(?!^\[\[package\]\]).)*?^version\s*=\s*"([^"]+)"'
    $match = [regex]::Match($content, $pattern)
    if (-not $match.Success) {
        throw "Could not read Cargo.lock version for package $PackageName from $Path."
    }

    return $match.Groups[1].Value
}

function Invoke-VersionConsistencyCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    Write-Host "Checking version consistency..."
    $pyprojectVersion = Get-RequiredRegexValue `
        (Join-Path $RepoRoot "pyproject.toml") `
        '(?ms)^\[project\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"' `
        "pyproject version"
    $cargoVersion = Get-RequiredRegexValue `
        (Join-Path $RepoRoot "Cargo.toml") `
        '(?ms)^\[workspace\.package\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"' `
        "Cargo workspace version"
    $packageVersion = Get-RequiredRegexValue `
        (Join-Path $RepoRoot "python\arborist_mcp\_version.py") `
        '(?m)^__version__\s*=\s*"([^"]+)"' `
        "Python package version"

    if ($pyprojectVersion -ne $cargoVersion -or $pyprojectVersion -ne $packageVersion) {
        throw "Version mismatch: pyproject=$pyprojectVersion Cargo=$cargoVersion package=$packageVersion."
    }

    foreach ($packageName in @("arborist-core", "arborist-py")) {
        $lockVersion = Get-CargoLockPackageVersion `
            (Join-Path $RepoRoot "Cargo.lock") `
            $packageName
        if ($lockVersion -ne $cargoVersion) {
            throw "Version mismatch: Cargo workspace=$cargoVersion Cargo.lock $packageName=$lockVersion."
        }
    }
}

function Ensure-GatewayExtension {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python
    )

    if ($script:GatewayExtensionPrepared) {
        return
    }

    Invoke-NativeOrThrow "Building gateway extension..." "cargo" @("build", "--locked", "-p", "arborist-py")
    Invoke-ScriptOrThrow "Syncing gateway extension..." { & (Join-Path $PSScriptRoot "sync-extension.ps1") -SkipBuild }
    $script:GatewayExtensionPrepared = $true
}

function Invoke-TestSuiteCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ProfileMetadata
    )

    $suiteName = [string]$ProfileMetadata.suite
    if ([string]::IsNullOrWhiteSpace($suiteName)) {
        throw "Suite-backed check profiles must define a non-empty suite."
    }

    $syncExtension = [string]$ProfileMetadata.sync_extension
    if ([string]::IsNullOrWhiteSpace($syncExtension)) {
        $syncExtension = "auto"
    }

    $prepareExtension = $false
    if ($ProfileMetadata.PSObject.Properties.Name -contains "prepare_extension") {
        $prepareExtension = [bool]$ProfileMetadata.prepare_extension
    }
    if ($prepareExtension) {
        Ensure-GatewayExtension $Python
    }

    Invoke-ScriptOrThrow "Running test suite '$suiteName'..." {
        & (Join-Path $PSScriptRoot "test.ps1") -Python $Python -Suite $suiteName -SyncExtension $syncExtension -Quiet
    }
}

function Invoke-GatewaySmokeCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python
    )

    Ensure-GatewayExtension $Python
    Invoke-NativeOrThrow "Checking gateway CLI..." $Python @("-m", "arborist_mcp.gateway", "--help")
    Invoke-NativeOrThrow "Checking gateway version..." $Python @("-m", "arborist_mcp.gateway", "--version")
    Invoke-GatewayInitializeSmoke $Python
}

function Invoke-CheckProfile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProfileName,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ProfileSnapshot
    )

    $profileMetadata = Get-CheckProfileMetadata $ProfileSnapshot $ProfileName
    $entries = @($profileMetadata.entries)
    if ($entries.Count -gt 0) {
        foreach ($entry in $entries) {
            Invoke-CheckProfile $entry $Python $RepoRoot $ProfileSnapshot
        }
        return
    }

    $handler = [string]$profileMetadata.handler
    switch ($handler) {
        "sanity" {
            Invoke-PowerShellSyntaxCheck
            Invoke-VersionConsistencyCheck $RepoRoot
            return
        }
        "rust" {
            Invoke-NativeOrThrow "Checking Rust formatting..." "cargo" @("fmt", "--check")
            Invoke-ScriptOrThrow "Running Rust tests..." { & (Join-Path $PSScriptRoot "test.ps1") -Python $Python -Suite rust -Quiet }
            Invoke-NativeOrThrow "Running Rust clippy..." "cargo" @("clippy", "--locked", "--all-targets", "--", "-D", "warnings")
            return
        }
        "suite" {
            Invoke-TestSuiteCheck $Python $profileMetadata
            return
        }
        "gateway-smoke" {
            Invoke-GatewaySmokeCheck $Python
            return
        }
        default {
            throw "Leaf check profile '$ProfileName' is missing a script implementation."
        }
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$profileSnapshot = Get-CheckProfileSnapshot $Python $repoRoot

if ($ListProfiles) {
    foreach ($profileName in @($profileSnapshot.profile_order)) {
        $profileMetadata = Get-CheckProfileMetadata $profileSnapshot $profileName
        Write-Host ("{0,-16} {1}" -f $profileName, $profileMetadata.description)
    }
    return
}

$knownProfiles = @($profileSnapshot.profile_order)
$unknownProfiles = @(
    $Profile |
        Where-Object { $knownProfiles -notcontains $_ } |
        Select-Object -Unique
)
if ($unknownProfiles.Count -gt 0) {
    throw "Unknown profile name(s): $($unknownProfiles -join ', '). Use -ListProfiles to inspect supported values."
}

Push-Location $repoRoot
try {
    foreach ($profileName in $Profile) {
        Invoke-CheckProfile $profileName $Python $repoRoot $profileSnapshot
    }
} finally {
    Pop-Location
}

Write-Host "All requested checks passed."
