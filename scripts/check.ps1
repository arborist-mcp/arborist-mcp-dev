param(
    [string]$Python = "python",
    [string[]]$Profile = @("full"),
    [switch]$ListProfiles,
    [switch]$ShowPlan
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

function Get-CheckExecutionPlan {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [string[]]$ProfileNames
    )

    $manifestEmitter = Join-Path $RepoRoot "scripts\check_profile_manifest.py"
    $rawOutput = & $Python $manifestEmitter --plan @ProfileNames 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Check execution plan helper failed with exit code $LASTEXITCODE.`n$($rawOutput | Out-String)"
    }

    $plan = $rawOutput | Out-String | ConvertFrom-Json
    if ($null -eq $plan -or $null -eq $plan.steps) {
        throw "Check execution plan helper must define 'steps'."
    }

    return $plan
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

function Expand-SelectionArguments {
    param(
        [string[]]$Values
    )

    $expanded = @()
    foreach ($value in @($Values)) {
        foreach ($entry in ([string]$value).Split(',')) {
            $normalized = $entry.Trim()
            if (-not [string]::IsNullOrWhiteSpace($normalized)) {
                $expanded += $normalized
            }
        }
    }

    return $expanded
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

function Invoke-ToolCatalogSnapshotCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    Invoke-NativeOrThrow `
        "Checking tool catalog snapshot..." `
        $Python `
        @((Join-Path $RepoRoot "scripts\tool_catalog.py"), "--check")
}

function Ensure-GatewayExtension {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python
    )

    if ($script:GatewayExtensionPrepared) {
        return
    }

    Invoke-NativeOrThrow "Building gateway extension..." "cargo" @("build", "--locked", "-p", "arborist-py", "--features", "extension-module")
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
    Invoke-NativeOrThrow `
        "Checking gateway smoke path..." `
        $Python `
        @((Join-Path $PSScriptRoot "gateway_smoke.py"), "--python", $Python, "--require-core")
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
            Invoke-ToolCatalogSnapshotCheck $Python $RepoRoot
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

function Show-CheckExecutionPlan {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ExecutionPlan
    )

    foreach ($step in @($ExecutionPlan.steps)) {
        $profileName = [string]$step.profile
        $handler = [string]$step.handler
        $needsRust = [bool]$step.needs_rust
        $needsPython = [bool]$step.needs_python
        $requirements = @()
        if ($needsRust) {
            $requirements += "rust"
        }
        if ($needsPython) {
            $requirements += "python"
        }
        if ($requirements.Count -eq 0) {
            $requirements += "none"
        }

        $detail = $handler
        if ($step.PSObject.Properties.Name -contains "suite") {
            $detail = "$detail -> $($step.suite)"
        }
        if ($step.PSObject.Properties.Name -contains "prepare_extension" -and [bool]$step.prepare_extension) {
            $detail = "$detail -> prepare-extension"
        }

        Write-Host ("{0,-16} {1} [{2}]" -f $profileName, $detail, ($requirements -join "+"))
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$profileSnapshot = Get-CheckProfileSnapshot $Python $repoRoot
$Profile = Expand-SelectionArguments $Profile

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

$executionPlan = Get-CheckExecutionPlan $Python $repoRoot $Profile

if ($ShowPlan) {
    Show-CheckExecutionPlan $executionPlan
    return
}

Push-Location $repoRoot
try {
    foreach ($step in @($executionPlan.steps)) {
        $profileName = [string]$step.profile
        Invoke-CheckProfile $profileName $Python $repoRoot $profileSnapshot
    }
} finally {
    Pop-Location
}

Write-Host "All requested checks passed."
