param(
    [string]$Python = "python",
    [string[]]$Suite = @("inner-loop"),
    [string]$RustFilter,
    [switch]$Quiet,
    [switch]$ListSuites,
    [ValidateSet("auto", "always", "never")]
    [string]$SyncExtension = "auto"
)

$ErrorActionPreference = "Stop"
$script:GatewayExtensionSynced = $false

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

function Invoke-ScriptOrThrow {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Description,
        [Parameter(Mandatory = $true)]
        [scriptblock]$Script
    )

    if (-not $Quiet) {
        Write-Host $Description
    }
    & $Script
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Get-GatewayManifest {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $manifestPath = Join-Path $RepoRoot "tests\gateway_protocol\suites.json"
    $rawManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($null -eq $rawManifest -or $null -eq $rawManifest.suites -or $null -eq $rawManifest.groups) {
        throw "Gateway suite manifest must define 'suites' and 'groups'."
    }

    $suites = [ordered]@{}
    foreach ($property in $rawManifest.suites.PSObject.Properties) {
        $suiteName = $property.Name
        $metadata = $property.Value
        if ([string]::IsNullOrWhiteSpace($suiteName)) {
            throw "Gateway suite manifest contains a blank suite name."
        }
        if ($null -eq $metadata) {
            throw "Gateway suite '$suiteName' is missing metadata."
        }

        $moduleName = [string]$metadata.module
        $description = [string]$metadata.description
        $requiresExtension = $metadata.requires_extension

        if ([string]::IsNullOrWhiteSpace($moduleName)) {
            throw "Gateway suite '$suiteName' must define a non-empty module name."
        }
        if ([string]::IsNullOrWhiteSpace($description)) {
            throw "Gateway suite '$suiteName' must define a non-empty description."
        }
        if ($requiresExtension -isnot [bool]) {
            throw "Gateway suite '$suiteName' must define a boolean requires_extension flag."
        }

        $suites[$suiteName] = [pscustomobject]@{
            ModuleName = $moduleName
            Description = $description
            RequiresExtension = $requiresExtension
        }
    }

    $groups = [ordered]@{}
    foreach ($property in $rawManifest.groups.PSObject.Properties) {
        $groupName = $property.Name
        $metadata = $property.Value
        if ([string]::IsNullOrWhiteSpace($groupName)) {
            throw "Gateway suite manifest contains a blank group name."
        }
        if ($null -eq $metadata) {
            throw "Gateway group '$groupName' is missing metadata."
        }

        $description = [string]$metadata.description
        $entries = @($metadata.entries)
        if ([string]::IsNullOrWhiteSpace($description)) {
            throw "Gateway group '$groupName' must define a non-empty description."
        }
        if ($entries.Count -eq 0) {
            throw "Gateway group '$groupName' must define at least one entry."
        }

        $normalizedEntries = @()
        foreach ($entry in $entries) {
            $entryName = [string]$entry
            if ([string]::IsNullOrWhiteSpace($entryName)) {
                throw "Gateway group '$groupName' contains a blank entry."
            }
            $normalizedEntries += $entryName
        }

        $groups[$groupName] = [pscustomobject]@{
            Description = $description
            Entries = $normalizedEntries
        }
    }

    return [pscustomobject]@{
        Suites = $suites
        Groups = $groups
    }
}

function Resolve-GatewayEntry {
    param(
        [Parameter(Mandatory = $true)]
        [string]$EntryName,
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Suites,
        [Parameter(Mandatory = $true)]
        [System.Collections.IDictionary]$Groups,
        [string[]]$Stack = @()
    )

    if ($Suites.Contains($EntryName)) {
        $suite = $Suites[$EntryName]
        return [pscustomobject]@{
            Modules = @($suite.ModuleName)
            SuiteNames = @($EntryName)
            RequiresExtension = [bool]$suite.RequiresExtension
        }
    }

    if (-not $Groups.Contains($EntryName)) {
        throw "Unknown gateway suite or group: $EntryName"
    }
    if ($Stack -contains $EntryName) {
        throw "Gateway suite manifest contains a recursive group cycle: $($Stack + $EntryName -join ' -> ')"
    }

    $modules = @()
    $suiteNames = @()
    $requiresExtension = $false
    foreach ($childEntry in $Groups[$EntryName].Entries) {
        $resolved = Resolve-GatewayEntry $childEntry $Suites $Groups ($Stack + $EntryName)
        foreach ($moduleName in $resolved.Modules) {
            if ($modules -notcontains $moduleName) {
                $modules += $moduleName
            }
        }
        foreach ($suiteName in $resolved.SuiteNames) {
            if ($suiteNames -notcontains $suiteName) {
                $suiteNames += $suiteName
            }
        }
        if ($resolved.RequiresExtension) {
            $requiresExtension = $true
        }
    }

    return [pscustomobject]@{
        Modules = $modules
        SuiteNames = $suiteNames
        RequiresExtension = $requiresExtension
    }
}

function Ensure-GatewayExtension {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension,
        [Parameter(Mandatory = $true)]
        [bool]$Required
    )

    if ($SyncExtension -eq "never") {
        return
    }
    if ($SyncExtension -eq "auto" -and -not $Required) {
        return
    }
    if ($script:GatewayExtensionSynced) {
        return
    }

    Invoke-NativeOrThrow "Building gateway extension..." "cargo" @("build", "--locked", "-p", "arborist-py")
    Invoke-ScriptOrThrow "Syncing gateway extension..." { & (Join-Path $PSScriptRoot "sync-extension.ps1") -SkipBuild }
    $script:GatewayExtensionSynced = $true
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

function Invoke-GatewaySelection {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SelectionName,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$GatewayManifest,
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension
    )

    $resolved = Resolve-GatewayEntry $SelectionName $GatewayManifest.Suites $GatewayManifest.Groups
    Ensure-GatewayExtension $SyncExtension $resolved.RequiresExtension
    Invoke-GatewayModules $Python $resolved.Modules "Running gateway suite '$SelectionName'..."
}

function Invoke-PythonDiscovery {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension
    )

    Ensure-GatewayExtension $SyncExtension $true
    Invoke-NativeOrThrow "Running Python tests..." $Python @("-m", "unittest", "discover", "-s", "tests")
}

function Get-SuiteDescriptions {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$GatewayManifest
    )

    $descriptions = [ordered]@{
        "rust" = "Run all Rust tests via cargo test --locked."
        "python" = "Run full Python unittest discovery under tests/."
        "inner-loop" = "Run Rust tests plus the gateway-fast group for the default local loop."
        "all" = "Run Rust tests plus the full Python unittest discovery suite."
    }

    foreach ($groupName in $GatewayManifest.Groups.Keys) {
        $descriptions[$groupName] = $GatewayManifest.Groups[$groupName].Description
    }

    foreach ($suiteName in $GatewayManifest.Suites.Keys) {
        $descriptions[$suiteName] = $GatewayManifest.Suites[$suiteName].Description
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
        [pscustomobject]$GatewayManifest,
        [AllowEmptyString()]
        [string]$RustFilter,
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension
    )

    if ($SuiteName -eq "rust") {
        Invoke-RustTests $RustFilter
        return
    }

    if ($SuiteName -eq "python") {
        Invoke-PythonDiscovery $Python $SyncExtension
        return
    }

    if ($SuiteName -eq "inner-loop") {
        Invoke-RustTests $RustFilter
        Invoke-GatewaySelection "gateway-fast" $Python $GatewayManifest $SyncExtension
        return
    }

    if ($SuiteName -eq "all") {
        Invoke-RustTests $RustFilter
        Invoke-PythonDiscovery $Python $SyncExtension
        return
    }

    if ($GatewayManifest.Groups.Contains($SuiteName) -or $GatewayManifest.Suites.Contains($SuiteName)) {
        Invoke-GatewaySelection $SuiteName $Python $GatewayManifest $SyncExtension
        return
    }

    throw "Unknown suite: $SuiteName"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$gatewayManifest = Get-GatewayManifest $repoRoot
$suiteDescriptions = Get-SuiteDescriptions $gatewayManifest

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
        Invoke-NamedSuite $suiteName $Python $gatewayManifest $RustFilter $SyncExtension
    }
} finally {
    Pop-Location
}

if (-not $Quiet) {
    Write-Host "Requested test suite passed."
}
