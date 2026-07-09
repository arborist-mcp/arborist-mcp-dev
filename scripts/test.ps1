param(
    [string]$Python = "python",
    [string[]]$Suite = @("inner-loop"),
    [string]$RustFilter,
    [switch]$Quiet,
    [switch]$ListSuites,
    [switch]$ShowPlan,
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

function Get-PythonExecutionPlan {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot,
        [Parameter(Mandatory = $true)]
        [string[]]$SelectionNames
    )

    $manifestEmitter = Join-Path $RepoRoot "scripts\python_suite_manifest.py"
    $rawOutput = & $Python $manifestEmitter --plan @SelectionNames 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Python execution plan helper failed with exit code $LASTEXITCODE.`n$($rawOutput | Out-String)"
    }

    $plan = $rawOutput | Out-String | ConvertFrom-Json
    if ($null -eq $plan -or $null -eq $plan.steps) {
        throw "Python execution plan helper must define 'steps'."
    }

    return $plan
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

function Get-SuiteDescriptions {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [Parameter(Mandatory = $true)]
        [string]$RepoRoot
    )

    $manifestEmitter = Join-Path $RepoRoot "scripts\python_suite_manifest.py"
    $rawOutput = & $Python $manifestEmitter --descriptions 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Python suite description helper failed with exit code $LASTEXITCODE.`n$($rawOutput | Out-String)"
    }

    $descriptions = $rawOutput | Out-String | ConvertFrom-Json
    if ($null -eq $descriptions) {
        throw "Python suite description helper returned no data."
    }

    $table = [ordered]@{}
    foreach ($entry in @($descriptions)) {
        $name = [string]$entry.name
        $description = [string]$entry.description
        if ([string]::IsNullOrWhiteSpace($name) -or [string]::IsNullOrWhiteSpace($description)) {
            throw "Python suite description helper returned a blank entry."
        }
        $table[$name] = $description
    }

    return $table
}

function Show-PythonExecutionPlan {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ExecutionPlan
    )

    foreach ($step in @($ExecutionPlan.steps)) {
        $kind = [string]$step.kind
        $selectionNames = @($step.selection_names)
        if ($kind -eq "rust") {
            Write-Host ("rust    <- {0}" -f ($selectionNames -join ", "))
            continue
        }

        $moduleNames = @($step.module_names)
        $requiresExtension = [bool]$step.requires_extension
        $extensionLabel = if ($requiresExtension) { "extension" } else { "pure-python" }
        Write-Host ((
            "python  <- {0} [{1}; {2} module(s)]" -f
            ($selectionNames -join ", "),
            $extensionLabel,
            $moduleNames.Count
        ))
        foreach ($moduleName in $moduleNames) {
            Write-Host ("          {0}" -f $moduleName)
        }
    }
}

function Invoke-ExecutionPlan {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$ExecutionPlan,
        [Parameter(Mandatory = $true)]
        [string]$Python,
        [AllowEmptyString()]
        [string]$RustFilter,
        [Parameter(Mandatory = $true)]
        [string]$SyncExtension
    )

    foreach ($step in @($ExecutionPlan.steps)) {
        $kind = [string]$step.kind
        if ($kind -eq "rust") {
            Invoke-RustTests $RustFilter
            continue
        }

        $requiresExtension = [bool]$step.requires_extension
        Ensure-GatewayExtension $SyncExtension $requiresExtension
        $moduleNames = @($step.module_names)
        $selectionNames = @($step.selection_names)
        Invoke-PythonModules `
            $Python `
            $moduleNames `
            ("Running Python test plan for '{0}'..." -f ($selectionNames -join ", "))
    }
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$suiteDescriptions = Get-SuiteDescriptions $Python $repoRoot
$Suite = Expand-SelectionArguments $Suite

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

$executionPlan = Get-PythonExecutionPlan $Python $repoRoot $Suite

if ($ShowPlan) {
    Show-PythonExecutionPlan $executionPlan
    return
}

Push-Location $repoRoot
try {
    Invoke-ExecutionPlan $executionPlan $Python $RustFilter $SyncExtension
} finally {
    Pop-Location
}

if (-not $Quiet) {
    Write-Host "Requested test suite passed."
}
