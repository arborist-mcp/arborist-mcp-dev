param(
    [string]$Python = "python",
    [string[]]$Profile = @("full"),
    [switch]$ListProfiles
)

$ErrorActionPreference = "Stop"
$script:GatewayExtensionPrepared = $false

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

function Invoke-CheckProfile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ProfileName,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    switch ($ProfileName) {
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
        "gateway-fast" {
            Invoke-ScriptOrThrow "Running gateway fast suite..." { & (Join-Path $PSScriptRoot "test.ps1") -Python $Python -Suite gateway-fast -SyncExtension never -Quiet }
            return
        }
        "python-native" {
            Ensure-GatewayExtension $Python
            Invoke-ScriptOrThrow "Running full Python test suite..." { & (Join-Path $PSScriptRoot "test.ps1") -Python $Python -Suite python -SyncExtension never -Quiet }
            Invoke-NativeOrThrow "Checking gateway CLI..." $Python @("-m", "arborist_mcp.gateway", "--help")
            Invoke-NativeOrThrow "Checking gateway version..." $Python @("-m", "arborist_mcp.gateway", "--version")
            Invoke-GatewayInitializeSmoke $Python
            return
        }
        "full" {
            Invoke-CheckProfile "sanity" $Python $RepoRoot
            Invoke-CheckProfile "rust" $Python $RepoRoot
            Invoke-CheckProfile "python-native" $Python $RepoRoot
            return
        }
        default {
            throw "Unknown check profile: $ProfileName"
        }
    }
}

function Get-ProfileDescriptions {
    return [ordered]@{
        "sanity" = "Run PowerShell syntax and cross-file version consistency checks."
        "rust" = "Run cargo fmt --check, cargo test --locked, and cargo clippy --locked --all-targets -D warnings."
        "gateway-fast" = "Run the pure-Python gateway protocol suites that do not require the synced native extension."
        "python-native" = "Build and sync the native gateway extension, run full Python unittest discovery, and exercise gateway CLI/version/initialize smoke checks."
        "full" = "Run the complete local check gate: sanity, rust, and python-native."
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$profileDescriptions = Get-ProfileDescriptions

if ($ListProfiles) {
    foreach ($profileName in $profileDescriptions.Keys) {
        Write-Host ("{0,-16} {1}" -f $profileName, $profileDescriptions[$profileName])
    }
    return
}

$unknownProfiles = @(
    $Profile |
        Where-Object { -not $profileDescriptions.Contains($_) } |
        Select-Object -Unique
)
if ($unknownProfiles.Count -gt 0) {
    throw "Unknown profile name(s): $($unknownProfiles -join ', '). Use -ListProfiles to inspect supported values."
}

Push-Location $repoRoot
try {
    foreach ($profileName in $Profile) {
        Invoke-CheckProfile $profileName $Python $repoRoot
    }
} finally {
    Pop-Location
}

Write-Host "All requested checks passed."
