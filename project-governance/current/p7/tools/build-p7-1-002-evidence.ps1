[CmdletBinding()]
param(
    [string]$RepositoryRoot,
    [string]$OutputDirectory,
    [switch]$AllowNpmRegistryNetwork,
    [switch]$VerifyOnly,
    [ValidateRange(30, 3600)][int]$ProcessTimeoutSeconds = 1200,
    [ValidateRange(1, 16)][int]$NpmExtractionConcurrency = 8,
    [ValidateRange(30, 600)][int]$NpmPackageTimeoutSeconds = 120
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Assert-Condition([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Has-Property([object]$Object, [string]$Name) {
    return $null -ne $Object -and $null -ne $Object.PSObject.Properties[$Name]
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
}

function Write-Utf8NoBom([string]$Path, [string]$Content) {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function ConvertTo-ProcessArgument([string]$Value) {
    if ($null -eq $Value) { return '""' }
    if ($Value.Length -gt 0 -and $Value -notmatch '[\s"]') { return $Value }

    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    $slashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq [char]92) {
            $slashes++
            continue
        }
        if ($character -eq [char]34) {
            if ($slashes -gt 0) { [void]$builder.Append(([string][char]92) * ($slashes * 2)) }
            [void]$builder.Append([char]92)
            [void]$builder.Append([char]34)
        } else {
            if ($slashes -gt 0) { [void]$builder.Append(([string][char]92) * $slashes) }
            [void]$builder.Append($character)
        }
        $slashes = 0
    }
    if ($slashes -gt 0) { [void]$builder.Append(([string][char]92) * ($slashes * 2)) }
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Stop-ProcessTree([System.Diagnostics.Process]$Process) {
    try { if ($Process.HasExited) { return } } catch { return }
    $taskkill = Join-Path $env:SystemRoot 'System32\taskkill.exe'
    if (Test-Path -LiteralPath $taskkill -PathType Leaf) {
        $killStart = New-Object System.Diagnostics.ProcessStartInfo
        $killStart.FileName = $taskkill
        $killStart.Arguments = "/PID $($Process.Id) /T /F"
        $killStart.UseShellExecute = $false
        $killStart.CreateNoWindow = $true
        $killStart.RedirectStandardOutput = $true
        $killStart.RedirectStandardError = $true
        $killer = New-Object System.Diagnostics.Process
        $killer.StartInfo = $killStart
        try {
            if ($killer.Start()) {
                [void]$killer.StandardOutput.ReadToEndAsync()
                [void]$killer.StandardError.ReadToEndAsync()
                if (-not $killer.WaitForExit(15000)) {
                    try { $killer.Kill() } catch {}
                    [void]$killer.WaitForExit(5000)
                }
            }
        } finally { $killer.Dispose() }
    }
    try {
        if (-not $Process.HasExited) {
            $Process.Kill()
            [void]$Process.WaitForExit(10000)
        }
    } catch {}
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [ValidateRange(1, 3600)][int]$TimeoutSeconds = $ProcessTimeoutSeconds
    )

    Assert-Condition (Test-Path -LiteralPath $FilePath -PathType Leaf) "executable is missing: $FilePath"
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $FilePath
    $start.Arguments = (@($Arguments | ForEach-Object { ConvertTo-ProcessArgument ([string]$_) }) -join ' ')
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $start
    try {
        Assert-Condition $process.Start() "failed to start: $FilePath"
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            Stop-ProcessTree $process
            throw "external process exceeded hard timeout of $TimeoutSeconds seconds: $FilePath"
        }
        $process.WaitForExit()
        $stdout = $stdoutTask.Result
        $stderr = $stderrTask.Result
        return [pscustomobject][ordered]@{ ExitCode = $process.ExitCode; Stdout = $stdout; Stderr = $stderr }
    } finally {
        try { if (-not $process.HasExited) { Stop-ProcessTree $process } } catch {}
        $process.Dispose()
    }
}

function ConvertTo-Tsv {
    param([string[]]$Headers, [object[]]$Rows)
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add(($Headers -join "`t"))
    foreach ($row in $Rows) {
        $values = foreach ($header in $Headers) {
            $value = if (Has-Property $row $header) { [string]$row.$header } else { '' }
            Assert-Condition ($value -notmatch "[`t`r`n]") "TSV value contains a control delimiter: $header"
            $value
        }
        $lines.Add(($values -join "`t"))
    }
    return (($lines -join "`n") + "`n")
}

function Get-OrdinalSortedRows([object[]]$Rows) {
    $items = New-Object 'System.Collections.Generic.List[object]'
    foreach ($row in $Rows) { $items.Add($row) }
    $comparison = [System.Comparison[object]]{
        param($left, $right)
        $leftKey = @(
            [string]$left.ecosystem,
            [string]$left.name,
            [string]$left.version,
            [string]$left.lock_key,
            $(if (Has-Property $left 'license_file') { [string]$left.license_file } else { '' })
        ) -join [char]31
        $rightKey = @(
            [string]$right.ecosystem,
            [string]$right.name,
            [string]$right.version,
            [string]$right.lock_key,
            $(if (Has-Property $right 'license_file') { [string]$right.license_file } else { '' })
        ) -join [char]31
        return [StringComparer]::Ordinal.Compare($leftKey, $rightKey)
    }
    $items.Sort($comparison)
    return $items.ToArray()
}

function Assert-ExactOrdinalCoverage {
    param(
        [string[]]$Actual,
        [string[]]$Expected,
        [string]$Label,
        [bool]$AllowDuplicateActual = $false
    )
    $actualSet = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    $expectedSet = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($value in $Expected) {
        Assert-Condition (-not [string]::IsNullOrWhiteSpace($value)) "$Label expected set contains an empty key"
        Assert-Condition $expectedSet.Add($value) "$Label expected set contains a duplicate key: $value"
    }
    foreach ($value in $Actual) {
        Assert-Condition (-not [string]::IsNullOrWhiteSpace($value)) "$Label actual set contains an empty key"
        $added = $actualSet.Add($value)
        if (-not $AllowDuplicateActual) { Assert-Condition $added "$Label contains a duplicate key: $value" }
    }
    $missing = @($expectedSet | Where-Object { -not $actualSet.Contains($_) } | Sort-Object)
    $extra = @($actualSet | Where-Object { -not $expectedSet.Contains($_) } | Sort-Object)
    Assert-Condition ($missing.Count -eq 0 -and $extra.Count -eq 0) "$Label lock_key coverage mismatch; missing=$($missing -join ','); extra=$($extra -join ',')"
}

function Assert-TsvExactHeader([string]$Content, [string[]]$Expected, [string]$Label) {
    $firstLine = ($Content -split "`n", 2)[0].TrimEnd("`r")
    Assert-Condition ($firstLine -ceq ($Expected -join "`t")) "$Label exact header mismatch"
}

function Get-RelativePath([string]$BasePath, [string]$ChildPath) {
    $baseFull = [System.IO.Path]::GetFullPath($BasePath).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    $childFull = [System.IO.Path]::GetFullPath($ChildPath)
    $baseUri = New-Object System.Uri($baseFull)
    $childUri = New-Object System.Uri($childFull)
    return [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($childUri).ToString()).Replace('\', '/')
}

function Test-IsUnder([string]$ChildPath, [string]$ParentPath) {
    $child = [System.IO.Path]::GetFullPath($ChildPath).TrimEnd('\', '/')
    $parent = [System.IO.Path]::GetFullPath($ParentPath).TrimEnd('\', '/')
    return $child.Equals($parent, [StringComparison]::OrdinalIgnoreCase) -or
        $child.StartsWith($parent + [System.IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Get-TomlQuotedValue([string]$Block, [string]$Name, [bool]$Required) {
    $match = [regex]::Match($Block, "(?m)^$([regex]::Escape($Name))\s*=\s*`"([^`"\x5c]*)`"\s*$")
    if ($match.Success) { return $match.Groups[1].Value }
    if ($Required) { throw "Cargo.lock package block lacks $Name" }
    return ''
}

function Get-CargoLockPackages([string]$Path) {
    $text = [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
    Assert-Condition ($text -match '(?m)^version\s*=\s*4\s*$') 'Cargo.lock must use lock format version 4'
    Assert-Condition ($text -notmatch '(?m)^source\s*=\s*"git\+') 'Cargo.lock contains a Git source'
    $blocks = [regex]::Split($text, '(?m)^\[\[package\]\]\s*$')
    $rows = New-Object System.Collections.Generic.List[object]
    for ($index = 1; $index -lt $blocks.Count; $index++) {
        $name = Get-TomlQuotedValue $blocks[$index] 'name' $true
        $version = Get-TomlQuotedValue $blocks[$index] 'version' $true
        $source = Get-TomlQuotedValue $blocks[$index] 'source' $false
        $checksum = Get-TomlQuotedValue $blocks[$index] 'checksum' $false
        $rows.Add([pscustomobject][ordered]@{
            name = $name
            version = $version
            source = $source
            checksum = $checksum
            key = @($name, $version, $source) -join [char]31
        })
    }
    Assert-Condition ($rows.Count -gt 1) 'Cargo.lock contains no resolved dependency closure'
    return $rows.ToArray()
}

function Get-LicenseFileRows {
    param(
        [string]$Ecosystem,
        [string]$Name,
        [string]$Version,
        [string]$LockKey,
        [string]$PackageRoot,
        [string]$ExplicitLicenseFile,
        [string]$RepositoryLicense,
        [bool]$IsLocal
    )

    $candidates = New-Object System.Collections.Generic.List[object]
    foreach ($file in @(Get-ChildItem -LiteralPath $PackageRoot -Force -File | Where-Object {
        $_.Name -match '^(?i:(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)([._-].*)?)$'
    })) {
        $candidates.Add([pscustomobject]@{ Path = $file.FullName; Relative = $file.Name })
    }
    if (-not [string]::IsNullOrWhiteSpace($ExplicitLicenseFile)) {
        $explicit = if ([System.IO.Path]::IsPathRooted($ExplicitLicenseFile)) {
            [System.IO.Path]::GetFullPath($ExplicitLicenseFile)
        } else {
            [System.IO.Path]::GetFullPath((Join-Path $PackageRoot $ExplicitLicenseFile))
        }
        Assert-Condition (Test-IsUnder $explicit $PackageRoot) "license_file escapes package root: $Name@$Version"
        Assert-Condition (Test-Path -LiteralPath $explicit -PathType Leaf) "declared license_file is missing: $Name@$Version"
        $candidates.Add([pscustomobject]@{ Path = $explicit; Relative = Get-RelativePath $PackageRoot $explicit })
    }
    if ($IsLocal) {
        Assert-Condition (Test-Path -LiteralPath $RepositoryLicense -PathType Leaf) 'repository LICENSE is missing'
        $candidates.Add([pscustomobject]@{ Path = $RepositoryLicense; Relative = 'LICENSE' })
    }

    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    $result = New-Object System.Collections.Generic.List[object]
    foreach ($candidate in $candidates) {
        $identity = [System.IO.Path]::GetFullPath([string]$candidate.Path)
        if (-not $seen.Add($identity)) { continue }
        $result.Add([pscustomobject][ordered]@{
            ecosystem = $Ecosystem
            name = $Name
            version = $Version
            license_file = ([string]$candidate.Relative).Replace('\', '/')
            sha256 = Get-Sha256 $identity
            lock_key = $LockKey
            evidence_status = 'HASHED'
            reason = 'PACKAGED_LICENSE_FILE'
        })
    }
    return $result.ToArray()
}

function Get-Purl([string]$Ecosystem, [string]$Name, [string]$Version) {
    if ($Ecosystem -eq 'rust') {
        return "pkg:cargo/$([Uri]::EscapeDataString($Name))@$([Uri]::EscapeDataString($Version))"
    }
    if ($Name.StartsWith('@')) {
        $parts = $Name.Split('/', 2)
        Assert-Condition ($parts.Count -eq 2) "invalid scoped npm name: $Name"
        return "pkg:npm/$([Uri]::EscapeDataString($parts[0]))/$([Uri]::EscapeDataString($parts[1]))@$([Uri]::EscapeDataString($Version))"
    }
    return "pkg:npm/$([Uri]::EscapeDataString($Name))@$([Uri]::EscapeDataString($Version))"
}

function Assert-ExactPropertyNames([object]$Object, [string[]]$Expected, [string]$Label) {
    Assert-Condition ($null -ne $Object) "$Label is null"
    $actual = @($Object.PSObject.Properties | ForEach-Object { [string]$_.Name })
    Assert-ExactOrdinalCoverage $actual $Expected "$Label property names" $false
}

function Get-CycloneDxPropertyMap([object]$Component) {
    $result = @{}
    foreach ($property in @($Component.properties)) {
        Assert-ExactPropertyNames $property @('name','value') 'CycloneDX component property'
        $name = [string]$property.name
        Assert-Condition (-not $result.ContainsKey($name)) "duplicate CycloneDX component property: $name"
        $result[$name] = [string]$property.value
    }
    return $result
}

function Assert-CycloneDx16Shape([object]$Bom, [string[]]$ExpectedLockKeys, [int]$ExpectedCargoCount, [int]$ExpectedNpmCount) {
    Assert-ExactPropertyNames $Bom @('bomFormat','specVersion','version','metadata','components') 'CycloneDX root'
    Assert-Condition ([string]$Bom.bomFormat -ceq 'CycloneDX' -and [string]$Bom.specVersion -ceq '1.6' -and [int]$Bom.version -eq 1) 'CycloneDX 1.6 root identity mismatch'
    Assert-ExactPropertyNames $Bom.metadata @('lifecycles','properties') 'CycloneDX metadata'
    Assert-Condition (@($Bom.metadata.lifecycles).Count -eq 1 -and [string]$Bom.metadata.lifecycles[0].phase -ceq 'pre-build') 'CycloneDX lifecycle must be pre-build'
    $metadataProperties = @{}
    foreach ($property in @($Bom.metadata.properties)) {
        Assert-ExactPropertyNames $property @('name','value') 'CycloneDX metadata property'
        Assert-Condition (-not $metadataProperties.ContainsKey([string]$property.name)) "duplicate CycloneDX metadata property: $($property.name)"
        $metadataProperties[[string]$property.name] = [string]$property.value
    }
    Assert-ExactOrdinalCoverage @($metadataProperties.Keys) @(
        'rbcsp:p7:task','rbcsp:p7:inventoryModel','rbcsp:p7:zeroConsumerScope','rbcsp:p7:rustSbomCrossValidation'
    ) 'CycloneDX metadata properties' $false
    Assert-Condition ($metadataProperties['rbcsp:p7:task'] -ceq 'P7.1-002') 'CycloneDX task property mismatch'
    Assert-Condition ($metadataProperties['rbcsp:p7:inventoryModel'] -ceq 'ONE_COMPONENT_PER_LOCK_ENTRY') 'CycloneDX inventory model mismatch'
    Assert-Condition ($metadataProperties['rbcsp:p7:zeroConsumerScope'] -ceq 'ACTIVE_P7_TARGET_GRAPH_ONLY') 'CycloneDX zero-consumer scope mismatch'
    Assert-Condition ($metadataProperties['rbcsp:p7:rustSbomCrossValidation'] -ceq 'PASS_CARGO_CYCLONEDX_0.5.9') 'CycloneDX Rust cross-validation state mismatch'

    $components = @($Bom.components)
    Assert-Condition ($components.Count -eq $ExpectedLockKeys.Count) 'CycloneDX component count mismatch'
    $bomRefs = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    $lockKeys = New-Object System.Collections.Generic.List[string]
    $cargoCount = 0
    $npmCount = 0
    foreach ($component in $components) {
        $expectedNames = New-Object System.Collections.Generic.List[string]
        foreach ($name in @('type','bom-ref','name','version','scope','purl','licenses','properties')) { $expectedNames.Add($name) }
        if (Has-Property $component 'hashes') { $expectedNames.Add('hashes') }
        if (Has-Property $component 'externalReferences') { $expectedNames.Add('externalReferences') }
        Assert-ExactPropertyNames $component $expectedNames.ToArray() "CycloneDX component $($component.name)"
        Assert-Condition ([string]$component.type -in @('application','library')) 'CycloneDX component type is invalid'
        Assert-Condition (-not [string]::IsNullOrWhiteSpace([string]$component.name) -and -not [string]::IsNullOrWhiteSpace([string]$component.version)) 'CycloneDX component name/version is empty'
        Assert-Condition ([string]$component.scope -in @('required','optional','excluded')) 'CycloneDX component scope is invalid'
        Assert-Condition ([string]$component.'bom-ref' -match '^urn:rbcsp:p7-1-002:[0-9a-f]{64}$') 'CycloneDX bom-ref format is invalid'
        Assert-Condition $bomRefs.Add([string]$component.'bom-ref') "duplicate CycloneDX bom-ref: $($component.'bom-ref')"
        Assert-Condition (@($component.licenses).Count -eq 1 -and -not [string]::IsNullOrWhiteSpace([string]$component.licenses[0].expression)) 'CycloneDX component license expression is missing'
        Assert-ExactPropertyNames $component.licenses[0] @('expression') 'CycloneDX license expression'
        if (Has-Property $component 'hashes') {
            foreach ($hash in @($component.hashes)) {
                Assert-ExactPropertyNames $hash @('alg','content') 'CycloneDX component hash'
                $expectedLength = switch ([string]$hash.alg) { 'SHA-256' { 64 } 'SHA-384' { 96 } 'SHA-512' { 128 } default { 0 } }
                Assert-Condition ($expectedLength -gt 0 -and [string]$hash.content -cmatch "^[0-9A-F]{$expectedLength}$") 'CycloneDX component hash shape is invalid'
            }
        }
        if (Has-Property $component 'externalReferences') {
            foreach ($reference in @($component.externalReferences)) {
                Assert-ExactPropertyNames $reference @('type','url') 'CycloneDX external reference'
                Assert-Condition ([string]$reference.type -ceq 'website' -and [string]$reference.url -match '^https://') 'CycloneDX external reference must be an HTTPS registry metadata website'
                Assert-Condition ([string]$reference.url -notmatch '\.tgz(?:$|[?#])') 'CycloneDX must not publish registry tarball URLs'
            }
        }
        $propertyMap = Get-CycloneDxPropertyMap $component
        Assert-ExactOrdinalCoverage @($propertyMap.Keys) @(
            'rbcsp:p7:ecosystem','rbcsp:p7:dependencyScope','rbcsp:p7:direct','rbcsp:p7:lockKey','rbcsp:p7:licenseDecision'
        ) 'CycloneDX component properties' $false
        Assert-Condition ($propertyMap['rbcsp:p7:dependencyScope'] -in @('runtime','dev')) 'CycloneDX dependency scope property is invalid'
        Assert-Condition ($propertyMap['rbcsp:p7:direct'] -in @('true','false')) 'CycloneDX direct property is invalid'
        Assert-Condition ($propertyMap['rbcsp:p7:licenseDecision'] -in @('APPROVED','PLATFORM_EXCLUDED')) 'CycloneDX license decision property is invalid'
        if ($propertyMap['rbcsp:p7:licenseDecision'] -eq 'PLATFORM_EXCLUDED') { Assert-Condition ([string]$component.scope -eq 'excluded') 'platform-excluded component scope must be excluded' }
        $lockKeys.Add($propertyMap['rbcsp:p7:lockKey'])
        if ($propertyMap['rbcsp:p7:ecosystem'] -eq 'rust') {
            $cargoCount++
            Assert-Condition ([string]$component.purl -like 'pkg:cargo/*') 'Rust CycloneDX purl is invalid'
        } elseif ($propertyMap['rbcsp:p7:ecosystem'] -eq 'npm') {
            $npmCount++
            Assert-Condition ([string]$component.purl -like 'pkg:npm/*') 'npm CycloneDX purl is invalid'
        } else { throw 'CycloneDX ecosystem property is invalid' }
    }
    Assert-ExactOrdinalCoverage $lockKeys.ToArray() $ExpectedLockKeys 'CycloneDX lock_key' $false
    Assert-Condition ($cargoCount -eq $ExpectedCargoCount -and $npmCount -eq $ExpectedNpmCount) 'CycloneDX ecosystem counts mismatch'
}

function Add-CargoPurlsFromCycloneDx([object]$Component, [System.Collections.Generic.HashSet[string]]$Destination) {
    if ($null -eq $Component) { return }
    if (Has-Property $Component 'purl' -and [string]$Component.purl -like 'pkg:cargo/*') {
        $basePurl = ([string]$Component.purl -split '[?#]', 2)[0]
        [void]$Destination.Add([Uri]::UnescapeDataString($basePurl))
    }
    if (Has-Property $Component 'components') {
        foreach ($child in @($Component.components)) { Add-CargoPurlsFromCycloneDx $child $Destination }
    }
}

$canonicalRepo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
if (-not [string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    $requestedRepo = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    Assert-Condition ($requestedRepo.Equals($canonicalRepo, [StringComparison]::OrdinalIgnoreCase)) 'RepositoryRoot must be the canonical repository containing this builder'
}
$repo = $canonicalRepo
$p7Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$canonicalOutput = [System.IO.Path]::GetFullPath((Join-Path $p7Root 'evidence\p7-1-002'))
if (-not [string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $requestedOutput = [System.IO.Path]::GetFullPath($OutputDirectory)
    Assert-Condition ($requestedOutput.Equals($canonicalOutput, [StringComparison]::OrdinalIgnoreCase)) 'OutputDirectory must be the exact approved P7.1-002 evidence directory'
}
$output = $canonicalOutput

$cargoLockPath = Join-Path $repo 'Cargo.lock'
$frontendRoot = Join-Path $repo 'frontend'
$frontendLockPath = Join-Path $frontendRoot 'package-lock.json'
$denyPath = Join-Path $repo 'deny.toml'
$contractPath = Join-Path $p7Root '02a-p7-1-002-dependency-lock-license-sbom-baseline.json'
$repositoryLicense = Join-Path $repo 'LICENSE'
$cargoCacheRoot = Join-Path $repo '.cache\p7-1-002'
$cargoHome = Join-Path $cargoCacheRoot 'cargo'
$rustupHome = Join-Path $cargoCacheRoot 'rustup'
$cargoTarget = Join-Path $cargoCacheRoot 'target'
$cargoExe = Join-Path $cargoHome 'bin\cargo.exe'
$cargoCyclonedxExe = Join-Path $cargoCacheRoot 'tools\cargo-cyclonedx\bin\cargo-cyclonedx.exe'
$nodeBundle = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64'
$nodeRoot = Join-Path $nodeBundle 'runtime\node-v24.18.0-win-x64'
$nodeExe = Join-Path $nodeRoot 'node.exe'
$pacotePath = Join-Path $nodeRoot 'node_modules\npm\node_modules\pacote'
$spdxParserPath = Join-Path $nodeRoot 'node_modules\npm\node_modules\spdx-expression-parse'
$npmCache = Join-Path $nodeBundle 'npm-cache'
$workParent = Join-Path $cargoCacheRoot 'evidence-work'
$workRoot = Join-Path $workParent ("run-$PID-$([Guid]::NewGuid().ToString('N'))")
$npmExtractRoot = Join-Path $workRoot 'npm-packages'
$helperPath = Join-Path $workRoot 'p7-1-002-evidence-helper.cjs'

foreach ($required in @($cargoLockPath, $frontendLockPath, $denyPath, $contractPath, $repositoryLicense, $cargoExe, $cargoCyclonedxExe, $nodeExe)) {
    Assert-Condition (Test-Path -LiteralPath $required -PathType Leaf) "required input is missing: $required"
}
foreach ($requiredDirectory in @($pacotePath, $spdxParserPath, $npmCache)) {
    Assert-Condition (Test-Path -LiteralPath $requiredDirectory -PathType Container) "required pinned Node resource is missing: $requiredDirectory"
}
[System.IO.Directory]::CreateDirectory($workParent) | Out-Null
$primaryError = $null
$scratchCleanupFailure = $null
try {
[System.IO.Directory]::CreateDirectory($workRoot) | Out-Null
[System.IO.Directory]::CreateDirectory($npmExtractRoot) | Out-Null
$helperSource = @'
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

function fail(message) { throw new Error(message); }
function sha256File(file) { return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex').toUpperCase(); }
function licenseFiles(root) {
  return fs.readdirSync(root, { withFileTypes: true })
    .filter(d => d.isFile() && /^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)([._-].*)?$/i.test(d.name))
    .map(d => ({ licenseFile: d.name.split(String.fromCharCode(92)).join('/'), sha256: sha256File(path.join(root, d.name)) }))
    .sort((a, b) => a.licenseFile < b.licenseFile ? -1 : a.licenseFile > b.licenseFile ? 1 : 0);
}
function npmNameFromLockKey(lockKey) {
  const marker = 'node_modules/';
  const index = lockKey.lastIndexOf(marker);
  if (index < 0) fail(`unsupported npm lock key: ${lockKey}`);
  return lockKey.slice(index + marker.length);
}
function integrityDetails(value) {
  if (typeof value !== 'string' || !value.trim()) fail('registry npm package lacks integrity');
  const token = value.trim().split(/\s+/)[0];
  const dash = token.indexOf('-');
  if (dash <= 0) fail(`invalid npm integrity: ${value}`);
  const algorithm = token.slice(0, dash).toLowerCase();
  const bytes = Buffer.from(token.slice(dash + 1), 'base64');
  const expected = { sha256: 32, sha384: 48, sha512: 64 }[algorithm];
  if (!expected || bytes.length !== expected) fail(`unsupported npm integrity: ${value}`);
  return { algorithm, hex: bytes.toString('hex').toUpperCase() };
}
function normalizeLicense(expression) {
  if (typeof expression !== 'string' || !expression.trim()) return '';
  const value = expression.trim();
  const slash = value.match(/^([A-Za-z0-9.+-]+)\/([A-Za-z0-9.+-]+)$/);
  return slash ? `${slash[1]} OR ${slash[2]}` : value;
}
function expressionAllowed(node, allowed, restricted, scope) {
  if (node.license) {
    if (!allowed.has(node.license)) return false;
    if (node.exception) return false;
    if (restricted.has(node.license) && scope !== 'dev') return false;
    return true;
  }
  if (node.conjunction === 'and') return expressionAllowed(node.left, allowed, restricted, scope) && expressionAllowed(node.right, allowed, restricted, scope);
  if (node.conjunction === 'or') return expressionAllowed(node.left, allowed, restricted, scope) || expressionAllowed(node.right, allowed, restricted, scope);
  return false;
}

function exactDependencyMap(label, manifest, lockRoot) {
  const hasManifest = Object.prototype.hasOwnProperty.call(manifest, label);
  const hasLock = Object.prototype.hasOwnProperty.call(lockRoot, label);
  if (hasManifest !== hasLock) fail(`frontend manifest/lock root ${label} presence mismatch`);
  const manifestMap = manifest[label] || {};
  const lockMap = lockRoot[label] || {};
  if (typeof manifestMap !== 'object' || Array.isArray(manifestMap) || typeof lockMap !== 'object' || Array.isArray(lockMap)) fail(`frontend ${label} must be objects`);
  const manifestKeys = Object.keys(manifestMap).sort();
  const lockKeys = Object.keys(lockMap).sort();
  if (manifestKeys.length !== lockKeys.length || manifestKeys.some((key, index) => key !== lockKeys[index])) fail(`frontend manifest/lock root ${label} name set mismatch`);
  for (const key of manifestKeys) {
    if (typeof manifestMap[key] !== 'string' || typeof lockMap[key] !== 'string' || manifestMap[key] !== lockMap[key]) fail(`frontend manifest/lock root ${label} value mismatch: ${key}`);
    if (/^npm:/i.test(manifestMap[key])) fail(`npm alias dependency is explicitly unsupported in the active graph: ${label}/${key}`);
  }
}

async function boundedMap(items, concurrency, action) {
  const results = new Array(items.length);
  const failures = [];
  let nextIndex = 0;
  async function runWorker() {
    while (true) {
      const index = nextIndex++;
      if (index >= items.length) return;
      try { results[index] = await action(items[index]); }
      catch (error) { failures.push(`${items[index].lockKey}: ${error && error.message ? error.message : String(error)}`); }
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, () => runWorker()));
  if (failures.length) fail(`npm package verification failures (${failures.length}):\n${failures.sort().join('\n')}`);
  return results;
}

async function npmMode(args) {
  const [repoRoot, frontendRoot, lockPath, cache, extractRoot, pacotePath, allowNetwork, concurrencyValue, timeoutValue] = args;
  const concurrency = Number(concurrencyValue);
  const packageTimeoutMs = Number(timeoutValue) * 1000;
  if (!Number.isInteger(concurrency) || concurrency < 1 || concurrency > 16) fail('invalid npm extraction concurrency');
  if (!Number.isInteger(packageTimeoutMs) || packageTimeoutMs < 30000 || packageTimeoutMs > 600000) fail('invalid npm package timeout');
  const pacote = require(pacotePath);
  const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  if (lock.lockfileVersion !== 3 || !lock.packages || Array.isArray(lock.packages)) fail('frontend package-lock must be v3 with a packages object');
  const rootEntry = lock.packages[''];
  if (!rootEntry || typeof rootEntry.name !== 'string' || typeof rootEntry.version !== 'string') fail('frontend lock root entry is incomplete');
  const packageJson = JSON.parse(fs.readFileSync(path.join(frontendRoot, 'package.json'), 'utf8'));
  exactDependencyMap('dependencies', packageJson, rootEntry);
  exactDependencyMap('devDependencies', packageJson, rootEntry);
  exactDependencyMap('optionalDependencies', packageJson, rootEntry);
  const directNames = new Set([
    ...Object.keys(rootEntry.dependencies || {}),
    ...Object.keys(rootEntry.devDependencies || {}),
    ...Object.keys(rootEntry.optionalDependencies || {})
  ]);
  if (packageJson.name !== rootEntry.name || packageJson.version !== rootEntry.version) fail('frontend root package metadata does not match its lock entry');
  if (typeof rootEntry.license === 'string' && normalizeLicense(rootEntry.license) !== normalizeLicense(packageJson.license)) fail('frontend root package license does not match its lock entry');
  const rootLicense = path.join(repoRoot, 'LICENSE');
  const rootComponent = {
    lockKey: '.', name: rootEntry.name, version: rootEntry.version, scope: 'runtime', direct: true,
    source: 'repository:frontend/package.json', integrity: 'NOT_APPLICABLE_LOCAL', integrityAlgorithm: '', integrityHex: '',
    licenseExpression: normalizeLicense(packageJson.license), metadataUrl: 'repository:frontend/package.json',
    licenseFiles: [{ licenseFile: 'LICENSE', sha256: sha256File(rootLicense) }]
  };
  const registryEntries = Object.keys(lock.packages).filter(lockKey => lockKey !== '').sort().map(lockKey => ({ lockKey, entry: lock.packages[lockKey] }));
  const registryComponents = await boundedMap(registryEntries, concurrency, async ({ lockKey, entry }) => {
    if (entry.link) fail(`linked npm package is outside the P7.1-002 active registry graph: ${lockKey}`);
    const installedName = npmNameFromLockKey(lockKey);
    const expectedName = entry.name || installedName;
    if (expectedName !== installedName) fail(`npm alias package is explicitly unsupported: ${lockKey} -> ${expectedName}`);
    if (typeof entry.version !== 'string' || !entry.version) fail(`npm lock entry lacks version: ${lockKey}`);
    if (typeof entry.resolved !== 'string' || !entry.resolved.startsWith('https://registry.npmjs.org/')) fail(`npm lock entry is not pinned to the official registry: ${lockKey}`);
    const integrity = integrityDetails(entry.integrity);
    const destination = path.join(extractRoot, crypto.createHash('sha256').update(lockKey).digest('hex'));
    fs.rmSync(destination, { recursive: true, force: true });
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(new Error(`npm package verification exceeded ${packageTimeoutMs / 1000}s`)), packageTimeoutMs);
    try {
      await pacote.extract(entry.resolved, destination, {
        cache, integrity: entry.integrity, offline: allowNetwork !== 'true', preferOffline: true, signal: controller.signal
      });
    } finally { clearTimeout(timeout); }
    const extractedPackageJson = JSON.parse(fs.readFileSync(path.join(destination, 'package.json'), 'utf8'));
    if (extractedPackageJson.name !== expectedName || extractedPackageJson.version !== entry.version) fail(`extracted npm metadata mismatch: ${lockKey}`);
    if (typeof entry.license === 'string' && normalizeLicense(entry.license) !== normalizeLicense(extractedPackageJson.license)) fail(`extracted npm license metadata mismatch: ${lockKey}`);
    const topLevelKey = `node_modules/${installedName}`;
    return {
      lockKey, name: expectedName, version: entry.version,
      scope: entry.dev === true ? 'dev' : 'runtime',
      direct: lockKey === topLevelKey && directNames.has(installedName),
      source: 'registry:npmjs', integrity: entry.integrity,
      integrityAlgorithm: integrity.algorithm, integrityHex: integrity.hex,
      licenseExpression: normalizeLicense(extractedPackageJson.license),
      metadataUrl: `https://registry.npmjs.org/${encodeURIComponent(expectedName)}/${encodeURIComponent(entry.version)}`,
      licenseFiles: licenseFiles(destination)
    };
  });
  const components = [rootComponent, ...registryComponents].sort((a, b) => a.lockKey < b.lockKey ? -1 : a.lockKey > b.lockKey ? 1 : 0);
  process.stdout.write(JSON.stringify({ lockfileVersion: lock.lockfileVersion, packageCount: Object.keys(lock.packages).length, components }));
}

function licensesMode(args) {
  const [inputPath, parserPath] = args;
  const parse = require(parserPath);
  const input = JSON.parse(fs.readFileSync(inputPath, 'utf8'));
  const restricted = new Set(['MPL-2.0', 'CC-BY-4.0']);
  const results = input.components.map(component => {
    const allowedValues = input.allowedLicenseIdsByEcosystem[component.ecosystem];
    if (!Array.isArray(allowedValues)) fail(`license policy is missing ecosystem: ${component.ecosystem}`);
    const allowed = new Set(allowedValues);
    const normalized = normalizeLicense(component.licenseExpression);
    if (!normalized) return { identity: component.identity, normalizedExpression: '', decision: 'UNKNOWN', reason: 'EMPTY_LICENSE_EXPRESSION' };
    try {
      const parsed = parse(normalized);
      const approved = expressionAllowed(parsed, allowed, restricted, component.scope);
      return {
        identity: component.identity,
        normalizedExpression: normalized,
        decision: approved ? 'APPROVED' : 'DENIED',
        reason: approved ? 'SPDX_POLICY_SATISFIED' : 'SPDX_POLICY_OR_SCOPE_NOT_SATISFIED'
      };
    } catch (error) {
      return { identity: component.identity, normalizedExpression: normalized, decision: 'UNKNOWN', reason: `INVALID_SPDX:${error.message}` };
    }
  });
  process.stdout.write(JSON.stringify({ results }));
}

(async () => {
  const [mode, ...args] = process.argv.slice(2);
  if (mode === 'npm') await npmMode(args);
  else if (mode === 'licenses') licensesMode(args);
  else fail(`unknown helper mode: ${mode}`);
})().catch(error => { console.error(error && error.stack ? error.stack : String(error)); process.exit(1); });
'@
Write-Utf8NoBom $helperPath ($helperSource.Replace("`r`n", "`n") + "`n")

$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$env:CARGO_TARGET_DIR = $cargoTarget
$env:Path = (Join-Path $cargoHome 'bin') + [System.IO.Path]::PathSeparator + $env:Path
$env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'
$env:CARGO_NET_OFFLINE = 'true'
$cargoMetadataResult = Invoke-CapturedProcess -FilePath $cargoExe -Arguments @(
    'metadata', '--locked', '--offline', '--format-version', '1'
) -WorkingDirectory $repo
Assert-Condition ($cargoMetadataResult.ExitCode -eq 0) "cargo metadata --locked --offline failed: $($cargoMetadataResult.Stderr.Trim())"
try { $cargoMetadata = $cargoMetadataResult.Stdout | ConvertFrom-Json }
catch { throw "cargo metadata returned invalid JSON: $($_.Exception.Message)" }
Assert-Condition ([int]$cargoMetadata.version -eq 1) 'cargo metadata format version mismatch'
Assert-Condition ($null -ne $cargoMetadata.resolve) 'cargo metadata has no resolved graph'

$cargoLockPackages = @(Get-CargoLockPackages $cargoLockPath)
$metadataByKey = @{}
$metadataById = @{}
foreach ($package in @($cargoMetadata.packages)) {
    $source = if ($null -eq $package.source) { '' } else { [string]$package.source }
    $key = @([string]$package.name, [string]$package.version, $source) -join [char]31
    Assert-Condition (-not $metadataByKey.ContainsKey($key)) "duplicate cargo metadata identity: $key"
    $metadataByKey[$key] = $package
    $cargoId = [string]$package.id
    Assert-Condition (-not $metadataById.ContainsKey($cargoId)) "duplicate cargo metadata package id: $cargoId"
    $metadataById[$cargoId] = $package
}
$lockByKey = @{}
foreach ($package in $cargoLockPackages) {
    Assert-Condition (-not $lockByKey.ContainsKey([string]$package.key)) "duplicate Cargo.lock identity: $($package.name)@$($package.version)"
    $lockByKey[[string]$package.key] = $package
}
Assert-Condition ($metadataByKey.Count -eq $lockByKey.Count) "Cargo.lock/metadata package count mismatch: $($lockByKey.Count)/$($metadataByKey.Count)"
foreach ($key in $lockByKey.Keys) { Assert-Condition $metadataByKey.ContainsKey($key) "Cargo.lock package is absent from metadata: $key" }
foreach ($key in $metadataByKey.Keys) { Assert-Condition $lockByKey.ContainsKey($key) "cargo metadata package is absent from Cargo.lock: $key" }

$nodeById = @{}
foreach ($node in @($cargoMetadata.resolve.nodes)) { $nodeById[[string]$node.id] = $node }
$runtimeReach = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
$devReach = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
$directIds = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
$runtimeQueue = New-Object 'System.Collections.Generic.Queue[string]'
$devQueue = New-Object 'System.Collections.Generic.Queue[string]'
$workspaceIds = @($cargoMetadata.workspace_members | ForEach-Object { [string]$_ })
Assert-Condition ($workspaceIds.Count -eq 1) 'P7.1-002 must have exactly one dependency-resolution workspace member'
foreach ($rootId in $workspaceIds) {
    Assert-Condition $nodeById.ContainsKey($rootId) "workspace member is absent from resolved graph: $rootId"
    [void]$runtimeReach.Add($rootId)
    [void]$directIds.Add($rootId)
    $runtimeQueue.Enqueue($rootId)
    foreach ($dependency in @($nodeById[$rootId].deps)) {
        [void]$directIds.Add([string]$dependency.pkg)
        $kinds = @($dependency.dep_kinds)
        $hasDev = @($kinds | Where-Object { [string]$_.kind -eq 'dev' }).Count -gt 0
        if ($hasDev -and $devReach.Add([string]$dependency.pkg)) { $devQueue.Enqueue([string]$dependency.pkg) }
    }
}
while ($runtimeQueue.Count -gt 0) {
    $id = $runtimeQueue.Dequeue()
    foreach ($dependency in @($nodeById[$id].deps)) {
        $kinds = @($dependency.dep_kinds)
        $hasNonDev = $kinds.Count -eq 0 -or @($kinds | Where-Object { [string]$_.kind -ne 'dev' }).Count -gt 0
        if ($hasNonDev -and $runtimeReach.Add([string]$dependency.pkg)) { $runtimeQueue.Enqueue([string]$dependency.pkg) }
    }
}
while ($devQueue.Count -gt 0) {
    $id = $devQueue.Dequeue()
    foreach ($dependency in @($nodeById[$id].deps)) {
        $kinds = @($dependency.dep_kinds)
        $hasNonDev = $kinds.Count -eq 0 -or @($kinds | Where-Object { [string]$_.kind -ne 'dev' }).Count -gt 0
        if ($hasNonDev -and $devReach.Add([string]$dependency.pkg)) { $devQueue.Enqueue([string]$dependency.pkg) }
    }
}

$lockedRows = New-Object System.Collections.Generic.List[object]
$metadataRows = New-Object System.Collections.Generic.List[object]
$licenseSeedRows = New-Object System.Collections.Generic.List[object]
$licenseHashRows = New-Object System.Collections.Generic.List[object]
$sbomSeedRows = New-Object System.Collections.Generic.List[object]
$cargoRegistrySourceRoot = Join-Path $cargoHome 'registry\src'
$expectedLocalManifest = [System.IO.Path]::GetFullPath((Join-Path $repo 'crates\dependency-baseline\Cargo.toml'))

foreach ($lockPackage in $cargoLockPackages) {
    $package = $metadataByKey[[string]$lockPackage.key]
    $id = [string]$package.id
    $scope = if ($runtimeReach.Contains($id)) { 'runtime' } elseif ($devReach.Contains($id)) { 'dev' } else { '' }
    Assert-Condition ($scope -ne '') "resolved Cargo package has no runtime/dev reachability: $($package.name)@$($package.version)"
    $isLocal = $null -eq $package.source
    $checksum = [string]$lockPackage.checksum
    $manifestPath = [System.IO.Path]::GetFullPath([string]$package.manifest_path)
    $packageRoot = Split-Path -Parent $manifestPath
    if ($isLocal) {
        Assert-Condition ($manifestPath.Equals($expectedLocalManifest, [StringComparison]::OrdinalIgnoreCase)) "unexpected local Cargo package: $manifestPath"
        Assert-Condition ([string]::IsNullOrEmpty($checksum)) "local Cargo package unexpectedly has a registry checksum: $($package.name)"
        $source = 'repository:crates/dependency-baseline/Cargo.toml'
        $checksumValue = 'NOT_APPLICABLE_LOCAL'
        $metadataUrl = $source
    } else {
        Assert-Condition ([string]$package.source -eq 'registry+https://github.com/rust-lang/crates.io-index') "non-crates.io Cargo source: $($package.name)"
        Assert-Condition ($checksum -match '^[0-9a-fA-F]{64}$') "Cargo registry checksum is invalid: $($package.name)@$($package.version)"
        if (Has-Property $package 'checksum') {
            Assert-Condition ([string]$package.checksum -eq $checksum) "Cargo.lock/metadata checksum mismatch: $($package.name)@$($package.version)"
        }
        Assert-Condition (Test-IsUnder $packageRoot $cargoRegistrySourceRoot) "Cargo registry manifest is outside the pinned Cargo source cache: $($package.name)"
        $source = [string]$package.source
        $checksumValue = $checksum.ToLowerInvariant()
        $metadataUrl = "https://crates.io/api/v1/crates/$([Uri]::EscapeDataString([string]$package.name))/$([Uri]::EscapeDataString([string]$package.version))"
    }
    $lockKey = "cargo:$($package.name)@$($package.version):$(if ($isLocal) { 'local' } else { 'crates.io' })"
    $licenseExpression = if ($null -eq $package.license) { '' } else { [string]$package.license }
    $direct = $directIds.Contains($id)
    $lockedRows.Add([pscustomobject][ordered]@{
        ecosystem = 'rust'; name = [string]$package.name; version = [string]$package.version
        scope = $scope; direct = $direct.ToString().ToLowerInvariant(); source = $source
        checksum = $checksumValue; lock_key = $lockKey
    })
    $metadataRows.Add([pscustomobject][ordered]@{
        ecosystem = 'rust'; name = [string]$package.name; version = [string]$package.version
        metadata_url = $metadataUrl; license_expression = $licenseExpression
        verification = 'PASS'; lock_key = $lockKey
    })
    $identity = "rust|$lockKey"
    $licenseSeedRows.Add([pscustomobject][ordered]@{
        identity = $identity; ecosystem = 'rust'; name = [string]$package.name; version = [string]$package.version
        scope = $scope; direct = $direct.ToString().ToLowerInvariant(); lock_key = $lockKey
        licenseExpression = $licenseExpression; cargo_id = $id
    })
    $cargoLicenseFileRows = @(Get-LicenseFileRows -Ecosystem 'rust' -Name ([string]$package.name) -Version ([string]$package.version) `
        -LockKey $lockKey -PackageRoot $packageRoot -ExplicitLicenseFile $(if ($null -eq $package.license_file) { '' } else { [string]$package.license_file }) `
        -RepositoryLicense $repositoryLicense -IsLocal $isLocal)
    if ($cargoLicenseFileRows.Count -eq 0) {
        $cargoLicenseFileRows = @([pscustomobject][ordered]@{
            ecosystem = 'rust'; name = [string]$package.name; version = [string]$package.version
            license_file = 'NO_PACKAGED_LICENSE_FILE'; sha256 = 'NOT_APPLICABLE'; lock_key = $lockKey
            evidence_status = 'ABSENT_WITH_EXPLICIT_DECISION'; reason = 'NO_LICENSE_FILE_IN_VERIFIED_PACKAGE_ARCHIVE'
        })
    }
    foreach ($row in $cargoLicenseFileRows) { $licenseHashRows.Add($row) }
    $sbomSeedRows.Add([pscustomobject][ordered]@{
        identity = $identity; ecosystem = 'rust'; name = [string]$package.name; version = [string]$package.version
        scope = $scope; direct = $direct; lock_key = $lockKey; source = $source; checksum = $checksumValue
        hashAlgorithm = $(if ($isLocal) { '' } else { 'SHA-256' }); hashValue = $(if ($isLocal) { '' } else { $checksum.ToUpperInvariant() })
        metadata_url = $metadataUrl; license_expression = $licenseExpression
    })
}

$npmResult = Invoke-CapturedProcess -FilePath $nodeExe -Arguments @(
    $helperPath, 'npm', $repo, $frontendRoot, $frontendLockPath, $npmCache, $npmExtractRoot, $pacotePath,
    $AllowNpmRegistryNetwork.IsPresent.ToString().ToLowerInvariant(), [string]$NpmExtractionConcurrency, [string]$NpmPackageTimeoutSeconds
) -WorkingDirectory $repo
Assert-Condition ($npmResult.ExitCode -eq 0) "npm lock/package evidence extraction failed: $($npmResult.Stderr.Trim())"
try { $npmEvidence = $npmResult.Stdout | ConvertFrom-Json }
catch { throw "normalized npm evidence is invalid JSON: $($_.Exception.Message)" }
Assert-Condition ([int]$npmEvidence.lockfileVersion -eq 3) 'normalized npm lock version mismatch'
Assert-Condition ([int]$npmEvidence.packageCount -eq @($npmEvidence.components).Count) 'npm lock/component one-to-one count mismatch'

foreach ($component in @($npmEvidence.components)) {
    $lockKey = "npm:$([string]$component.lockKey)"
    $identity = "npm|$lockKey"
    $lockedRows.Add([pscustomobject][ordered]@{
        ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
        scope = [string]$component.scope; direct = ([bool]$component.direct).ToString().ToLowerInvariant()
        source = [string]$component.source; checksum = [string]$component.integrity; lock_key = $lockKey
    })
    $metadataRows.Add([pscustomobject][ordered]@{
        ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
        metadata_url = [string]$component.metadataUrl; license_expression = [string]$component.licenseExpression
        verification = 'PASS'; lock_key = $lockKey
    })
    $licenseSeedRows.Add([pscustomobject][ordered]@{
        identity = $identity; ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
        scope = [string]$component.scope; direct = ([bool]$component.direct).ToString().ToLowerInvariant(); lock_key = $lockKey
        licenseExpression = [string]$component.licenseExpression; cargo_id = ''
    })
    $npmLicenseFiles = @($component.licenseFiles)
    if ($npmLicenseFiles.Count -eq 0) {
        $licenseHashRows.Add([pscustomobject][ordered]@{
            ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
            license_file = 'NO_PACKAGED_LICENSE_FILE'; sha256 = 'NOT_APPLICABLE'; lock_key = $lockKey
            evidence_status = 'ABSENT_WITH_EXPLICIT_DECISION'; reason = 'NO_LICENSE_FILE_IN_VERIFIED_PACKAGE_ARCHIVE'
        })
    } else {
        foreach ($file in $npmLicenseFiles) {
            Assert-Condition ([string]$file.sha256 -match '^[0-9A-F]{64}$') "invalid npm packaged license hash: $($component.name)@$($component.version)"
            $licenseHashRows.Add([pscustomobject][ordered]@{
                ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
                license_file = [string]$file.licenseFile; sha256 = [string]$file.sha256; lock_key = $lockKey
                evidence_status = 'HASHED'; reason = 'PACKAGED_LICENSE_FILE'
            })
        }
    }
    $hashAlgorithm = if ([string]::IsNullOrEmpty([string]$component.integrityAlgorithm)) { '' } else {
        switch ([string]$component.integrityAlgorithm) { 'sha256' { 'SHA-256' } 'sha384' { 'SHA-384' } 'sha512' { 'SHA-512' } default { throw 'unsupported normalized npm integrity algorithm' } }
    }
    $sbomSeedRows.Add([pscustomobject][ordered]@{
        identity = $identity; ecosystem = 'npm'; name = [string]$component.name; version = [string]$component.version
        scope = [string]$component.scope; direct = [bool]$component.direct; lock_key = $lockKey
        source = [string]$component.source; checksum = [string]$component.integrity
        hashAlgorithm = $hashAlgorithm; hashValue = [string]$component.integrityHex
        metadata_url = [string]$component.metadataUrl; license_expression = [string]$component.licenseExpression
    })
}

$denyText = [System.IO.File]::ReadAllText($denyPath, [System.Text.Encoding]::UTF8)
$licenseSection = [regex]::Match($denyText, '(?ms)^\[licenses\]\s*(.*?)(?=^\[[^\r\n]+\]|\z)')
Assert-Condition $licenseSection.Success 'deny.toml lacks [licenses] policy'
$allowMatch = [regex]::Match($licenseSection.Groups[1].Value, '(?ms)^allow\s*=\s*\[(.*?)\]')
Assert-Condition $allowMatch.Success 'deny.toml lacks licenses.allow'
$allowedLicenseIds = @([regex]::Matches($allowMatch.Groups[1].Value, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
Assert-Condition ($allowedLicenseIds.Count -gt 0) 'deny.toml licenses.allow is empty'

try { $contract = [System.IO.File]::ReadAllText($contractPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json }
catch { throw "P7.1-002 contract is invalid JSON: $($_.Exception.Message)" }
Assert-Condition (Has-Property $contract 'licensePolicy') 'P7.1-002 contract lacks licensePolicy'
Assert-Condition ([string]$contract.licensePolicy.rustAllowSource -eq 'deny.toml') 'Rust license policy source must be deny.toml'
$npmAllowedLicenseIds = @($contract.licensePolicy.npmAllowed | ForEach-Object { [string]$_ } | Sort-Object -Unique)
Assert-Condition ($npmAllowedLicenseIds.Count -gt 0) 'contract npmAllowed is empty'
Assert-Condition ([string]$contract.licensePolicy.npmConditional.'MPL-2.0' -eq 'DEV_ONLY_UNMODIFIED_EXCLUDED_FROM_FINAL_RUNTIME_BUNDLES') 'MPL-2.0 conditional policy mismatch'
Assert-Condition ([string]$contract.licensePolicy.npmConditional.'CC-BY-4.0' -eq 'BUILD_ONLY_ATTRIBUTION_REQUIRED_IF_ENCOUNTERED') 'CC-BY-4.0 conditional policy mismatch'
Assert-Condition ([string]$contract.licensePolicy.unknownOrUnresolvedDecision -eq 'STOP') 'unknown license policy must STOP'

$graphSection = [regex]::Match($denyText, '(?ms)^\[graph\]\s*(.*?)(?=^\[[^\r\n]+\]|\z)')
Assert-Condition $graphSection.Success 'deny.toml lacks [graph] target policy'
$approvedTargets = @([regex]::Matches($graphSection.Groups[1].Value, 'triple\s*=\s*"([^"]+)"') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
$expectedTargets = @('x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu')
Assert-Condition ($approvedTargets.Count -eq $expectedTargets.Count) 'approved Cargo target count mismatch'
foreach ($target in $expectedTargets) { Assert-Condition ($approvedTargets -contains $target) "approved Cargo target is missing: $target" }

$activeTargetCargoIds = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
foreach ($target in $approvedTargets) {
    $filteredResult = Invoke-CapturedProcess -FilePath $cargoExe -Arguments @(
        'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', $target
    ) -WorkingDirectory $repo
    Assert-Condition ($filteredResult.ExitCode -eq 0) "cargo metadata target filter failed for $target`: $($filteredResult.Stderr.Trim())"
    try { $filteredMetadata = $filteredResult.Stdout | ConvertFrom-Json }
    catch { throw "cargo metadata target filter returned invalid JSON for $target`: $($_.Exception.Message)" }
    Assert-Condition ($null -ne $filteredMetadata.resolve) "target-filtered cargo metadata lacks a resolved graph: $target"
    foreach ($package in @($filteredMetadata.packages)) {
        $filteredId = [string]$package.id
        Assert-Condition $metadataById.ContainsKey($filteredId) "target-filtered cargo package id is absent from the canonical lock metadata: $filteredId"
        [void]$activeTargetCargoIds.Add($filteredId)
    }
}

$platformExclusions = @($contract.licensePolicy.rustPlatformExcluded)
Assert-Condition ($platformExclusions.Count -eq 1) 'rustPlatformExcluded must contain exactly the approved exception'
$platformExclusion = $platformExclusions[0]
Assert-Condition ([string]$platformExclusion.name -eq 'webpki-root-certs') 'platform exclusion package name mismatch'
Assert-Condition ([string]$platformExclusion.version -eq '1.0.8') 'platform exclusion package version mismatch'
Assert-Condition ([string]$platformExclusion.license -eq 'CDLA-Permissive-2.0') 'platform exclusion license mismatch'
Assert-Condition ([string]$platformExclusion.decision -eq 'PLATFORM_EXCLUDED') 'platform exclusion decision mismatch'
Assert-Condition ([string]$platformExclusion.basis -eq 'UNREACHABLE_FROM_X86_64_PC_WINDOWS_MSVC_AND_X86_64_UNKNOWN_LINUX_GNU_ACTIVE_TARGET_GRAPHS') 'platform exclusion basis mismatch'
$platformExclusionIdentity = "rust|cargo:$($platformExclusion.name)@$($platformExclusion.version):crates.io"
$platformTargetEvidence = "ABSENT_CARGO_ID_FROM_CARGO_METADATA_FILTER_PLATFORM:$($approvedTargets -join ',')"

$licenseInputPath = Join-Path $workRoot 'license-input.json'
$licenseInput = [pscustomobject][ordered]@{
    allowedLicenseIdsByEcosystem = [pscustomobject][ordered]@{
        rust = @($allowedLicenseIds)
        npm = @($npmAllowedLicenseIds)
    }
    components = @($licenseSeedRows.ToArray() | ForEach-Object {
        [pscustomobject][ordered]@{ identity = $_.identity; ecosystem = $_.ecosystem; scope = $_.scope; licenseExpression = $_.licenseExpression }
    })
}
$licenseInputJson = $licenseInput | ConvertTo-Json -Depth 10
Write-Utf8NoBom $licenseInputPath ($licenseInputJson.Replace("`r`n", "`n") + "`n")
$licenseResult = Invoke-CapturedProcess -FilePath $nodeExe -Arguments @(
    $helperPath, 'licenses', $licenseInputPath, $spdxParserPath
) -WorkingDirectory $repo
Assert-Condition ($licenseResult.ExitCode -eq 0) "SPDX license evaluation failed: $($licenseResult.Stderr.Trim())"
try { $licenseEvaluation = $licenseResult.Stdout | ConvertFrom-Json }
catch { throw "normalized SPDX evaluation is invalid JSON: $($_.Exception.Message)" }
Assert-Condition (@($licenseEvaluation.results).Count -eq $licenseSeedRows.Count) 'license evaluation/component count mismatch'
$licenseEvaluationByIdentity = @{}
foreach ($result in @($licenseEvaluation.results)) {
    Assert-Condition (-not $licenseEvaluationByIdentity.ContainsKey([string]$result.identity)) "duplicate license evaluation identity: $($result.identity)"
    $licenseEvaluationByIdentity[[string]$result.identity] = $result
}
$licenseRows = New-Object System.Collections.Generic.List[object]
$licenseFailures = New-Object System.Collections.Generic.List[string]
$platformExclusionUsed = 0
foreach ($seed in $licenseSeedRows) {
    Assert-Condition $licenseEvaluationByIdentity.ContainsKey([string]$seed.identity) "missing license evaluation: $($seed.identity)"
    $evaluation = $licenseEvaluationByIdentity[[string]$seed.identity]
    $decision = [string]$evaluation.decision
    $decisionReason = [string]$evaluation.reason
    $targetEvidence = 'NOT_APPLICABLE'
    if ([string]$seed.identity -eq $platformExclusionIdentity) {
        Assert-Condition ([string]$seed.ecosystem -eq 'rust') 'platform exclusion is not a Rust component'
        Assert-Condition ([string]$seed.name -eq [string]$platformExclusion.name -and [string]$seed.version -eq [string]$platformExclusion.version) 'platform exclusion identity drift'
        Assert-Condition ([string]$evaluation.normalizedExpression -eq [string]$platformExclusion.license) 'platform exclusion declared license drift'
        Assert-Condition (-not [string]::IsNullOrWhiteSpace([string]$seed.cargo_id)) 'platform-excluded component lacks canonical Cargo package id evidence'
        Assert-Condition (-not $activeTargetCargoIds.Contains([string]$seed.cargo_id)) 'platform-excluded Cargo package id is reachable from an approved target graph'
        $decision = 'PLATFORM_EXCLUDED'
        $decisionReason = [string]$platformExclusion.basis
        $targetEvidence = $platformTargetEvidence
        $platformExclusionUsed++
    } elseif ($decision -eq 'APPROVED' -and [string]$seed.ecosystem -eq 'npm' -and [string]$evaluation.normalizedExpression -eq 'MPL-2.0') {
        Assert-Condition ([string]$seed.scope -eq 'dev') "MPL-2.0 npm component is not dev-only: $($seed.name)@$($seed.version)"
        $decisionReason = [string]$contract.licensePolicy.npmConditional.'MPL-2.0'
    }
    if ($decision -notin @('APPROVED', 'PLATFORM_EXCLUDED')) {
        $licenseFailures.Add("$($seed.ecosystem)/$($seed.name)@$($seed.version) [$($seed.scope)]: $decision $decisionReason '$($evaluation.normalizedExpression)'")
    }
    $licenseRows.Add([pscustomobject][ordered]@{
        ecosystem = [string]$seed.ecosystem; name = [string]$seed.name; version = [string]$seed.version
        license_expression = [string]$evaluation.normalizedExpression; decision = $decision
        scope = [string]$seed.scope; direct = [string]$seed.direct; lock_key = [string]$seed.lock_key
        decision_reason = $decisionReason; target_evidence = $targetEvidence
    })
}
Assert-Condition ($platformExclusionUsed -eq 1) 'approved platform exclusion was not used exactly once'
if ($licenseFailures.Count -gt 0) {
    throw "unknown or denied dependency licenses; no final evidence was written:`n$($licenseFailures -join "`n")"
}

$licenseByIdentity = @{}
foreach ($row in $licenseRows) {
    $licenseIdentity = "$($row.ecosystem)|$($row.lock_key)"
    Assert-Condition (-not $licenseByIdentity.ContainsKey($licenseIdentity)) "duplicate license identity: $licenseIdentity"
    $licenseByIdentity[$licenseIdentity] = $row
}
foreach ($row in $metadataRows) {
    $identity = "$($row.ecosystem)|$($row.lock_key)"
    Assert-Condition $licenseByIdentity.ContainsKey($identity) "metadata row has no license decision: $identity"
    $row.license_expression = [string]$licenseByIdentity[$identity].license_expression
}

$lockedRowsSorted = @(Get-OrdinalSortedRows $lockedRows.ToArray())
$metadataRowsSorted = @(Get-OrdinalSortedRows $metadataRows.ToArray())
$licenseRowsSorted = @(Get-OrdinalSortedRows $licenseRows.ToArray())
$licenseHashRowsSorted = @(Get-OrdinalSortedRows $licenseHashRows.ToArray())
$sbomSeedsSorted = @(Get-OrdinalSortedRows $sbomSeedRows.ToArray())

$expectedComponentCount = $cargoLockPackages.Count + [int]$npmEvidence.packageCount
Assert-Condition ($lockedRowsSorted.Count -eq $expectedComponentCount) 'combined locked inventory count mismatch'
Assert-Condition ($metadataRowsSorted.Count -eq $expectedComponentCount) 'official metadata inventory count mismatch'
Assert-Condition ($licenseRowsSorted.Count -eq $expectedComponentCount) 'license decision inventory count mismatch'

$canonicalLockKeys = New-Object System.Collections.Generic.List[string]
foreach ($package in $cargoLockPackages) {
    $sourceKind = if ([string]::IsNullOrEmpty([string]$package.source)) { 'local' } else {
        Assert-Condition ([string]$package.source -eq 'registry+https://github.com/rust-lang/crates.io-index') "canonical Cargo lock source is unsupported: $($package.source)"
        'crates.io'
    }
    $canonicalLockKeys.Add("cargo:$($package.name)@$($package.version):$sourceKind")
}
foreach ($component in @($npmEvidence.components)) { $canonicalLockKeys.Add("npm:$([string]$component.lockKey)") }
$canonicalLockKeyArray = $canonicalLockKeys.ToArray()
Assert-ExactOrdinalCoverage @($lockedRowsSorted | ForEach-Object { [string]$_.lock_key }) $canonicalLockKeyArray 'locked-components.tsv' $false
Assert-ExactOrdinalCoverage @($metadataRowsSorted | ForEach-Object { [string]$_.lock_key }) $canonicalLockKeyArray 'official-metadata-sources.tsv' $false
Assert-ExactOrdinalCoverage @($licenseRowsSorted | ForEach-Object { [string]$_.lock_key }) $canonicalLockKeyArray 'license-allowlist.tsv' $false
Assert-ExactOrdinalCoverage @($licenseHashRowsSorted | ForEach-Object { [string]$_.lock_key }) $canonicalLockKeyArray 'license-file-hashes.tsv' $true
Assert-ExactOrdinalCoverage @($sbomSeedsSorted | ForEach-Object { [string]$_.lock_key }) $canonicalLockKeyArray 'SBOM seed inventory' $false
foreach ($row in $licenseHashRowsSorted) {
    if ([string]$row.evidence_status -eq 'HASHED') {
        Assert-Condition ([string]$row.license_file -ne 'NO_PACKAGED_LICENSE_FILE' -and [string]$row.sha256 -match '^[0-9A-F]{64}$' -and [string]$row.reason -eq 'PACKAGED_LICENSE_FILE') "invalid hashed license evidence: $($row.lock_key)"
    } else {
        Assert-Condition ([string]$row.evidence_status -eq 'ABSENT_WITH_EXPLICIT_DECISION' -and [string]$row.license_file -eq 'NO_PACKAGED_LICENSE_FILE' -and [string]$row.sha256 -eq 'NOT_APPLICABLE' -and [string]$row.reason -eq 'NO_LICENSE_FILE_IN_VERIFIED_PACKAGE_ARCHIVE') "invalid no-license-file sentinel: $($row.lock_key)"
    }
}

$rejectedNames = @('directories', 'fastify', 'better-sqlite3', 'react-window', '@types/react-window')
$rejected = @($lockedRowsSorted | Where-Object { $rejectedNames -ccontains [string]$_.name })
$rejectedFound = @($rejected | ForEach-Object { [string]$_.name }) -join ','
Assert-Condition ($rejected.Count -eq 0) "rejected dependency exists in active locked graph: $rejectedFound"
$embedCount = @($lockedRowsSorted | Where-Object { $_.ecosystem -eq 'rust' -and @('include_dir', 'rust-embed') -ccontains [string]$_.name }).Count
Assert-Condition ($embedCount -eq 1) "active Rust graph embed candidate count is $embedCount, expected 1"

$rustSbomCrossValidation = 'NOT_STARTED_REQUIRED_TOOL'
if (Test-Path -LiteralPath $cargoCyclonedxExe -PathType Leaf) {
    $versionResult = Invoke-CapturedProcess -FilePath $cargoCyclonedxExe -Arguments @('cyclonedx','--version') -WorkingDirectory $repo -TimeoutSeconds 60
    Assert-Condition ($versionResult.ExitCode -eq 0 -and $versionResult.Stdout.Trim() -ceq 'cargo-cyclonedx-cyclonedx 0.5.9') 'pinned cargo-cyclonedx version mismatch'
    $crossDirectory = Join-Path $workRoot 'cargo-cyclonedx-crosscheck'
    $crossProject = Join-Path $crossDirectory 'workspace'
    $crossCrate = Join-Path $crossProject 'crates\dependency-baseline'
    [System.IO.Directory]::CreateDirectory((Join-Path $crossCrate 'src')) | Out-Null
    [System.IO.File]::Copy((Join-Path $repo 'Cargo.toml'), (Join-Path $crossProject 'Cargo.toml'), $true)
    [System.IO.File]::Copy($cargoLockPath, (Join-Path $crossProject 'Cargo.lock'), $true)
    [System.IO.File]::Copy((Join-Path $repo 'crates\dependency-baseline\Cargo.toml'), (Join-Path $crossCrate 'Cargo.toml'), $true)
    [System.IO.File]::Copy((Join-Path $repo 'crates\dependency-baseline\src\lib.rs'), (Join-Path $crossCrate 'src\lib.rs'), $true)
    $cargoLockBeforeCross = Get-Sha256 $cargoLockPath
    $env:SOURCE_DATE_EPOCH = '0'
    $crossResult = Invoke-CapturedProcess -FilePath $cargoCyclonedxExe -Arguments @(
        'cyclonedx','--manifest-path',(Join-Path $crossCrate 'Cargo.toml'),
        '--format','json','--target','all','--all','--spec-version','1.5','--override-filename','rust','--quiet'
    ) -WorkingDirectory $crossProject
    Assert-Condition ($crossResult.ExitCode -eq 0) "cargo-cyclonedx Rust cross-check failed: $($crossResult.Stderr.Trim())"
    Assert-Condition ((Get-Sha256 $cargoLockPath) -eq $cargoLockBeforeCross) 'cargo-cyclonedx changed Cargo.lock'
    $crossFiles = @(Get-ChildItem -LiteralPath $crossProject -Recurse -File -Filter 'rust.json')
    Assert-Condition ($crossFiles.Count -eq 1) "cargo-cyclonedx produced $($crossFiles.Count) scratch SBOM files, expected 1"
    try { $crossSbom = [System.IO.File]::ReadAllText($crossFiles[0].FullName, [System.Text.Encoding]::UTF8) | ConvertFrom-Json }
    catch { throw "cargo-cyclonedx Rust cross-check JSON is invalid: $($_.Exception.Message)" }
    $crossCargoPurls = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    if (Has-Property $crossSbom.metadata 'component') { Add-CargoPurlsFromCycloneDx $crossSbom.metadata.component $crossCargoPurls }
    foreach ($component in @($crossSbom.components)) { Add-CargoPurlsFromCycloneDx $component $crossCargoPurls }
    $expectedCargoPurls = @($runtimeReach | ForEach-Object {
        $runtimePackage = $metadataById[[string]$_]
        [Uri]::UnescapeDataString((Get-Purl 'rust' ([string]$runtimePackage.name) ([string]$runtimePackage.version)))
    })
    $crossCargoPurlArray = @($crossCargoPurls | ForEach-Object { [string]$_ })
    Assert-ExactOrdinalCoverage $crossCargoPurlArray $expectedCargoPurls 'cargo-cyclonedx Rust purl cross-check' $false
    $rustSbomCrossValidation = 'PASS_CARGO_CYCLONEDX_0.5.9'
}
Assert-Condition ($rustSbomCrossValidation -ceq 'PASS_CARGO_CYCLONEDX_0.5.9') 'required cargo-cyclonedx Rust cross-validation did not pass'

$sbomComponents = New-Object System.Collections.Generic.List[object]
foreach ($seed in $sbomSeedsSorted) {
    $license = $licenseByIdentity[[string]$seed.identity]
    Assert-Condition ($null -ne $license) "SBOM component lacks license decision: $($seed.identity)"
    $bomRefHashInput = [System.Text.Encoding]::UTF8.GetBytes([string]$seed.identity)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try { $bomRefHash = ([BitConverter]::ToString($sha.ComputeHash($bomRefHashInput))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
    $component = [ordered]@{
        type = $(if ($seed.lock_key -in @('cargo:rbcsp-dependency-baseline@0.0.0:local', 'npm:.')) { 'application' } else { 'library' })
        'bom-ref' = "urn:rbcsp:p7-1-002:$bomRefHash"
        name = [string]$seed.name
        version = [string]$seed.version
        scope = $(if ([string]$license.decision -eq 'PLATFORM_EXCLUDED') { 'excluded' } elseif ([string]$seed.scope -eq 'dev') { 'optional' } else { 'required' })
        purl = Get-Purl ([string]$seed.ecosystem) ([string]$seed.name) ([string]$seed.version)
        licenses = @([pscustomobject][ordered]@{ expression = [string]$license.license_expression })
    }
    if (-not [string]::IsNullOrEmpty([string]$seed.hashAlgorithm)) {
        Assert-Condition ([string]$seed.hashValue -match '^[0-9A-F]{64,128}$') "SBOM component hash is invalid: $($seed.identity)"
        $component['hashes'] = @([pscustomobject][ordered]@{ alg = [string]$seed.hashAlgorithm; content = [string]$seed.hashValue })
    }
    if ([string]$seed.metadata_url -like 'https://*') {
        $references = New-Object System.Collections.Generic.List[object]
        $references.Add([pscustomobject][ordered]@{ type = 'website'; url = [string]$seed.metadata_url })
        if ([string]$seed.source -like 'https://*') { $references.Add([pscustomobject][ordered]@{ type = 'distribution'; url = [string]$seed.source }) }
        $component['externalReferences'] = $references.ToArray()
    }
    $component['properties'] = @(
        [pscustomobject][ordered]@{ name = 'rbcsp:p7:ecosystem'; value = [string]$seed.ecosystem },
        [pscustomobject][ordered]@{ name = 'rbcsp:p7:dependencyScope'; value = [string]$seed.scope },
        [pscustomobject][ordered]@{ name = 'rbcsp:p7:direct'; value = ([bool]$seed.direct).ToString().ToLowerInvariant() },
        [pscustomobject][ordered]@{ name = 'rbcsp:p7:lockKey'; value = [string]$seed.lock_key },
        [pscustomobject][ordered]@{ name = 'rbcsp:p7:licenseDecision'; value = [string]$license.decision }
    )
    $sbomComponents.Add([pscustomobject]$component)
}
Assert-Condition ($sbomComponents.Count -eq $expectedComponentCount) 'CycloneDX/component one-to-one count mismatch'
Assert-Condition (@($sbomComponents | Where-Object { $rejectedNames -ccontains [string]$_.name }).Count -eq 0) 'CycloneDX contains a rejected dependency'
Assert-Condition (@($sbomComponents | Where-Object { [string]$_.purl -like 'pkg:cargo/*' }).Count -eq $cargoLockPackages.Count) 'CycloneDX Cargo coverage mismatch'
Assert-Condition (@($sbomComponents | Where-Object { [string]$_.purl -like 'pkg:npm/*' }).Count -eq [int]$npmEvidence.packageCount) 'CycloneDX npm coverage mismatch'

$sbom = [pscustomobject][ordered]@{
    bomFormat = 'CycloneDX'
    specVersion = '1.6'
    version = 1
    metadata = [pscustomobject][ordered]@{
        lifecycles = @([pscustomobject][ordered]@{ phase = 'pre-build' })
        properties = @(
            [pscustomobject][ordered]@{ name = 'rbcsp:p7:task'; value = 'P7.1-002' },
            [pscustomobject][ordered]@{ name = 'rbcsp:p7:inventoryModel'; value = 'ONE_COMPONENT_PER_LOCK_ENTRY' },
            [pscustomobject][ordered]@{ name = 'rbcsp:p7:zeroConsumerScope'; value = 'ACTIVE_P7_TARGET_GRAPH_ONLY' },
            [pscustomobject][ordered]@{ name = 'rbcsp:p7:rustSbomCrossValidation'; value = $rustSbomCrossValidation }
        )
    }
    components = $sbomComponents.ToArray()
}

Assert-CycloneDx16Shape $sbom $canonicalLockKeyArray $cargoLockPackages.Count ([int]$npmEvidence.packageCount)
$lockedHeaders = @('ecosystem','name','version','scope','direct','source','checksum','lock_key')
$metadataHeaders = @('ecosystem','name','version','metadata_url','license_expression','verification','lock_key')
$licenseHeaders = @('ecosystem','name','version','license_expression','decision','scope','direct','lock_key','decision_reason','target_evidence')
$licenseHashHeaders = @('ecosystem','name','version','license_file','sha256','lock_key','evidence_status','reason')
$lockedTsv = ConvertTo-Tsv $lockedHeaders $lockedRowsSorted
$metadataTsv = ConvertTo-Tsv $metadataHeaders $metadataRowsSorted
$licensesTsv = ConvertTo-Tsv $licenseHeaders $licenseRowsSorted
$licenseHashesTsv = ConvertTo-Tsv $licenseHashHeaders $licenseHashRowsSorted
Assert-TsvExactHeader $lockedTsv $lockedHeaders 'locked-components.tsv'
Assert-TsvExactHeader $metadataTsv $metadataHeaders 'official-metadata-sources.tsv'
Assert-TsvExactHeader $licensesTsv $licenseHeaders 'license-allowlist.tsv'
Assert-TsvExactHeader $licenseHashesTsv $licenseHashHeaders 'license-file-hashes.tsv'
$sbomJson = (($sbom | ConvertTo-Json -Depth 30).Replace("`r`n", "`n") + "`n")
try { $roundTripSbom = $sbomJson | ConvertFrom-Json }
catch { throw "generated CycloneDX JSON failed strict round-trip parsing: $($_.Exception.Message)" }
Assert-CycloneDx16Shape $roundTripSbom $canonicalLockKeyArray $cargoLockPackages.Count ([int]$npmEvidence.packageCount)

$contents = [ordered]@{
    'locked-components.tsv' = $lockedTsv
    'official-metadata-sources.tsv' = $metadataTsv
    'license-allowlist.tsv' = $licensesTsv
    'license-file-hashes.tsv' = $licenseHashesTsv
    'initial-sbom.cdx.json' = $sbomJson
}
$repoPattern = [regex]::Escape($repo)
foreach ($entry in $contents.GetEnumerator()) {
    Assert-Condition (-not [regex]::IsMatch([string]$entry.Value, $repoPattern, [Text.RegularExpressions.RegexOptions]::IgnoreCase)) "absolute repository path leaked into $($entry.Key)"
    Assert-Condition ([string]$entry.Value -notmatch '(?i)[A-Z]:[\x5c/]+Users[\x5c/]') "absolute user path leaked into $($entry.Key)"
    Assert-Condition ([string]$entry.Value -notmatch '(?i)(^|[\x5c/])\.secrets([\x5c/]|$)') "private root leaked into $($entry.Key)"
    Assert-Condition ([string]$entry.Value -notmatch '(?i)https://[^\s"'']+\.tgz(?:[?#][^\s"'']*)?') "registry tarball URL leaked into $($entry.Key)"
}

if (-not $VerifyOnly.IsPresent) {
    [System.IO.Directory]::CreateDirectory($output) | Out-Null
    $temporaryFiles = New-Object System.Collections.Generic.List[object]
    try {
        foreach ($entry in $contents.GetEnumerator()) {
            $finalPath = Join-Path $output ([string]$entry.Key)
            $temporaryPath = "$finalPath.tmp.$PID"
            Write-Utf8NoBom $temporaryPath ([string]$entry.Value)
            $temporaryFiles.Add([pscustomobject]@{ Temporary = $temporaryPath; Final = $finalPath })
        }
        foreach ($file in $temporaryFiles) { Move-Item -LiteralPath $file.Temporary -Destination $file.Final -Force }
    } finally {
        foreach ($file in $temporaryFiles) {
            if (Test-Path -LiteralPath $file.Temporary) { Remove-Item -LiteralPath $file.Temporary -Force }
        }
    }
}

Write-Output 'p7_1_002_evidence=PASS'
Write-Output "write_mode=$(if ($VerifyOnly.IsPresent) { 'VERIFY_ONLY_FINAL_EVIDENCE_NOT_WRITTEN' } else { 'WRITE_CANONICAL_EVIDENCE' })"
Write-Output 'scratch_io=ISOLATED_AND_CLEANED_IN_FINALLY'
Write-Output 'registry_cache_io=PINNED_TASK_CACHE_MAY_BE_POPULATED'
Write-Output "cargo_lock_components=$($cargoLockPackages.Count)"
Write-Output "npm_lock_components=$([int]$npmEvidence.packageCount)"
Write-Output "combined_components=$expectedComponentCount"
Write-Output "license_file_hashes=$($licenseHashRowsSorted.Count)"
Write-Output 'cyclonedx_spec=1.6'
Write-Output 'rejected_components=0'
Write-Output 'unknown_or_denied_licenses=0'
Write-Output "platform_excluded_licenses=$platformExclusionUsed"
} catch {
    $primaryError = $_
} finally {
    $resolvedScratch = [System.IO.Path]::GetFullPath($workRoot)
    $resolvedScratchParent = [System.IO.Path]::GetFullPath($workParent).TrimEnd('\', '/')
    $actualScratchParent = [System.IO.Path]::GetDirectoryName($resolvedScratch).TrimEnd('\', '/')
    $scratchLeaf = [System.IO.Path]::GetFileName($resolvedScratch)
    $scratchBoundaryValid = $actualScratchParent.Equals($resolvedScratchParent, [StringComparison]::OrdinalIgnoreCase) -and
        $scratchLeaf -cmatch '^run-[0-9]+-[0-9a-f]{32}$'
    $cleanupDiagnostic = ''
    if (-not $scratchBoundaryValid) {
        $cleanupDiagnostic = 'scratch path failed exact parent/name safety validation; deletion was refused'
    } elseif ([System.IO.Directory]::Exists($resolvedScratch)) {
        $nodeCleanupSource = "const fs=require('fs');const p=process.argv[1];fs.rmSync(p,{recursive:true,force:true,maxRetries:8,retryDelay:125});if(fs.existsSync(p)){process.stderr.write('validated scratch still exists');process.exit(2)}"
        try {
            $cleanupResult = Invoke-CapturedProcess -FilePath $nodeExe -Arguments @('-e',$nodeCleanupSource,$resolvedScratch) -WorkingDirectory $repo -TimeoutSeconds 180
            if ($cleanupResult.ExitCode -ne 0) { $cleanupDiagnostic = "pinned Node scratch cleanup exited $($cleanupResult.ExitCode): $($cleanupResult.Stderr.Trim())" }
        } catch {
            $cleanupDiagnostic = "pinned Node scratch cleanup failed: $($_.Exception.Message)"
        }
    }
    if ([System.IO.Directory]::Exists($resolvedScratch)) {
        $scratchCleanupFailure = if ([string]::IsNullOrWhiteSpace($cleanupDiagnostic)) {
            'validated scratch directory still exists after pinned Node cleanup'
        } else { $cleanupDiagnostic }
    }
}

if ($null -ne $primaryError) {
    if ($null -ne $scratchCleanupFailure) { Write-Warning "secondary_scratch_cleanup_failure=$scratchCleanupFailure" }
    $PSCmdlet.ThrowTerminatingError($primaryError)
}
if ($null -ne $scratchCleanupFailure) { throw $scratchCleanupFailure }
