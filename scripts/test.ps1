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

function Get-PythonTestManifest {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $manifestEmitter = Join-Path $RepoRoot "scripts\python_suite_manifest.py"
    $rawOutput = & $Python $manifestEmitter 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Python test suite manifest helper failed with exit code $LASTEXITCODE.`n$($rawOutput | Out-String)"
    }

    $rawManifest = $rawOutput | Out-String | ConvertFrom-Json
    if ($null -eq $rawManifest -or $null -eq $rawManifest.suites -or $null -eq $rawManifest.groups) {
        throw "Python test suite manifest must define 'suites' and 'groups'."
    }

    $suites = [ordered]@{}
    foreach ($property in $rawManifest.suites.PSObject.Properties) {
        $suiteName = $property.Name
        $metadata = $property.Value
        if ([string]::IsNullOrWhiteSpace($suiteName)) {
            throw "Python test suite manifest contains a blank suite name."
        }
        if ($null -eq $metadata) {
            throw "Python test suite '$suiteName' is missing metadata."
        }

        $moduleName = [string]$metadata.module
        $description = [string]$metadata.description
        $requiresExtension = $metadata.requires_extension

        if ([string]::IsNullOrWhiteSpace($moduleName)) {
            throw "Python test suite '$suiteName' must define a non-empty module name."
        }
        if ([string]::IsNullOrWhiteSpace($description)) {
            throw "Python test suite '$suiteName' must define a non-empty description."
        }
        if ($requiresExtension -isnot [bool]) {
            throw "Python test suite '$suiteName' must define a boolean requires_extension flag."
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
            throw "Python test suite manifest contains a blank group name."
        }
        if ($null -eq $metadata) {
            throw "Python test group '$groupName' is missing metadata."
        }

        $description = [string]$metadata.description
        $entries = @($metadata.entries)
        $suiteNames = @($metadata.suite_names)
        $moduleNames = @($metadata.module_names)
        $requiresExtension = $metadata.requires_extension
        if ([string]::IsNullOrWhiteSpace($description)) {
            throw "Python test group '$groupName' must define a non-empty description."
        }
        if ($entries.Count -eq 0) {
            throw "Python test group '$groupName' must define at least one entry."
        }
        if ($suiteNames.Count -eq 0) {
            throw "Python test group '$groupName' must define at least one suite_name."
        }
        if ($moduleNames.Count -eq 0) {
            throw "Python test group '$groupName' must define at least one module_name."
        }
        if ($requiresExtension -isnot [bool]) {
            throw "Python test group '$groupName' must define a boolean requires_extension flag."
        }

        $normalizedEntries = @()
        foreach ($entry in $entries) {
            $entryName = [string]$entry
            if ([string]::IsNullOrWhiteSpace($entryName)) {
                throw "Python test group '$groupName' contains a blank entry."
            }
            $normalizedEntries += $entryName
        }

        $normalizedSuiteNames = @()
        foreach ($suiteName in $suiteNames) {
            $entryName = [string]$suiteName
            if ([string]::IsNullOrWhiteSpace($entryName)) {
                throw "Python test group '$groupName' contains a blank suite_name."
            }
            $normalizedSuiteNames += $entryName
        }

        $normalizedModuleNames = @()
        foreach ($moduleName in $moduleNames) {
            $entryName = [string]$moduleName
            if ([string]::IsNullOrWhiteSpace($entryName)) {
                throw "Python test group '$groupName' contains a blank module_name."
            }
            $normalizedModuleNames += $entryName
        }

        $groups[$groupName] = [pscustomobject]@{
            Description = $description
            Entries = $normalizedEntries
            SuiteNames = $normalizedSuiteNames
            ModuleNames = $normalizedModuleNames
            RequiresExtension = $requiresExtension
        }
    }

    return [pscustomobject]@{
        Suites = $suites
        Groups = $groups
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

function Invoke-PythonModules {
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

function Invoke-TestSelection {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SelectionName,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [pscustomobject]$PythonManifest,
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension
    )

    if ($PythonManifest.Suites.Contains($SelectionName)) {
        $suite = $PythonManifest.Suites[$SelectionName]
        Ensure-GatewayExtension $SyncExtension $suite.RequiresExtension
        Invoke-PythonModules $Python @($suite.ModuleName) "Running Python test suite '$SelectionName'..."
        return
    }

    if ($PythonManifest.Groups.Contains($SelectionName)) {
        $group = $PythonManifest.Groups[$SelectionName]
        Ensure-GatewayExtension $SyncExtension $group.RequiresExtension
        Invoke-PythonModules $Python $group.ModuleNames "Running Python test suite '$SelectionName'..."
        return
    }

    throw "Unknown Python test suite or group: $SelectionName"
}

function Get-SuiteDescriptions {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$PythonManifest
    )

    $descriptions = [ordered]@{
        "rust" = "Run all Rust tests via cargo test --locked."
        "python" = $PythonManifest.Groups["python"].Description
        "inner-loop" = "Run Rust tests plus the python-fast group for the default local loop."
        "all" = "Run Rust tests plus the full Python suite set."
    }

    foreach ($groupName in $PythonManifest.Groups.Keys) {
        if ($groupName -eq "python") {
            continue
        }
        $descriptions[$groupName] = $PythonManifest.Groups[$groupName].Description
    }

    foreach ($suiteName in $PythonManifest.Suites.Keys) {
        $descriptions[$suiteName] = $PythonManifest.Suites[$suiteName].Description
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
        [pscustomobject]$PythonManifest,
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
        Invoke-TestSelection "python" $Python $PythonManifest $SyncExtension
        return
    }

    if ($SuiteName -eq "inner-loop") {
        Invoke-RustTests $RustFilter
        Invoke-TestSelection "python-fast" $Python $PythonManifest $SyncExtension
        return
    }

    if ($SuiteName -eq "all") {
        Invoke-RustTests $RustFilter
        Invoke-TestSelection "python" $Python $PythonManifest $SyncExtension
        return
    }

    if ($PythonManifest.Groups.Contains($SuiteName) -or $PythonManifest.Suites.Contains($SuiteName)) {
        Invoke-TestSelection $SuiteName $Python $PythonManifest $SyncExtension
        return
    }

    throw "Unknown suite: $SuiteName"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$pythonManifest = Get-PythonTestManifest $Python $repoRoot
$suiteDescriptions = Get-SuiteDescriptions $pythonManifest

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
        Invoke-NamedSuite $suiteName $Python $pythonManifest $RustFilter $SyncExtension
    }
} finally {
    Pop-Location
}

if (-not $Quiet) {
    Write-Host "Requested test suite passed."
}
