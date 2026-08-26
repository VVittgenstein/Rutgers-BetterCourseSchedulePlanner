[CmdletBinding(DefaultParameterSetName = 'Verify')]
param(
    # Defaulted in the body: $PSScriptRoot is not available to parameter
    # default expressions under Windows PowerShell 5.1. Positions keep the
    # invocation surface the untagged parameters had.
    [Parameter(ParameterSetName = 'Verify', Position = 0)]
    [string]$ReleaseDirectory,

    [Parameter(ParameterSetName = 'Verify', Mandatory = $true, Position = 1)]
    [string]$SourceCommit,

    [Parameter(ParameterSetName = 'Verify', Mandatory = $true, Position = 2)]
    [long]$SourceDateEpoch,

    # Runs the frontend-capability release gate against in-memory fixtures
    # instead of verifying built archives. The gate is only evidence if it
    # can be executed where no archive exists: this is how CI proves the
    # required-slug semantics -- not just the file's syntax -- on every push.
    [Parameter(ParameterSetName = 'SelfTest', Mandatory = $true)]
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($PSCmdlet.ParameterSetName -ceq 'Verify') {
    if (-not $PSBoundParameters.ContainsKey('ReleaseDirectory')) {
        # Only when OMITTED. An explicitly passed empty value -- a wrapper's
        # unset variable -- must keep failing loudly, not verify the default
        # directory in place of the one the caller meant.
        $ReleaseDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) 'release\0.1.1'
    }
    if ($SourceCommit -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') {
        throw 'SourceCommit must be a full lowercase hexadecimal commit id.'
    }
    if ($SourceDateEpoch -lt 0) {
        throw 'SourceDateEpoch must be a non-negative integer.'
    }
}

$RepositoryRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$InputsPath = Join-Path $PSScriptRoot 'release-inputs.json'

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Get-Sha256 {
    param([string]$Path)
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Read-Json {
    param([string]$Path)
    try { Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json }
    catch { throw "Invalid JSON in $Path ($($_.Exception.Message))" }
}

function Sort-Ordinal {
    param([string[]]$Values)
    $copy = [string[]]@($Values)
    [System.Array]::Sort($copy, [System.StringComparer]::Ordinal)
    $copy
}

function Assert-Sequence {
    param([string[]]$Actual, [string[]]$Expected, [string]$Label)
    Assert-True ($Actual.Count -eq $Expected.Count) "$Label count mismatch: expected $($Expected.Count), found $($Actual.Count)."
    for ($index = 0; $index -lt $Expected.Count; $index += 1) {
        Assert-True ($Actual[$index] -ceq $Expected[$index]) "$Label mismatch: expected '$($Expected[$index])', found '$($Actual[$index])'."
    }
}

function Assert-SafeNames {
    param([string[]]$Names, [string]$Label)
    $exact = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
    $folded = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($name in $Names) {
        Assert-True ($name.Length -gt 0 -and -not $name.StartsWith('/') -and -not $name.EndsWith('/')) "$Label contains a rooted or directory entry: $name"
        Assert-True (-not $name.Contains('\') -and -not $name.Contains(':') -and $name -notmatch '(^|/)\.\.?(/|$)') "$Label contains an unsafe path: $name"
        Assert-True ($exact.Add($name) -and $folded.Add($name)) "$Label contains a duplicate or case-colliding path: $name"
    }
}

function Get-PackageFiles {
    param([string]$Root)
    $prefix = [System.IO.Path]::GetFullPath($Root).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    $files = foreach ($item in @(Get-ChildItem -LiteralPath $Root -Recurse -Force)) {
        Assert-True (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) "Package contains a reparse point: $($item.FullName)"
        if (-not $item.PSIsContainer) {
            $full = [System.IO.Path]::GetFullPath($item.FullName)
            Assert-True ($full.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) "Extracted path escaped package root: $full"
            $full.Substring($prefix.Length).Replace('\', '/')
        }
    }
    @(Sort-Ordinal @($files))
}

function Expand-ZipSafe {
    param([string]$Archive, [string]$Root, [string[]]$Expected)
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        $entries = @($zip.Entries)
        $names = @($entries | ForEach-Object { [string]$_.FullName })
        Assert-SafeNames $names 'Windows ZIP'
        Assert-Sequence @(Sort-Ordinal $names) $Expected 'Windows ZIP allowlist'
        $prefix = [System.IO.Path]::GetFullPath($Root).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
        foreach ($entry in $entries) {
            Assert-True ($entry.Name.Length -gt 0) "Windows ZIP contains a directory: $($entry.FullName)"
            $attrs = [System.BitConverter]::ToUInt32([System.BitConverter]::GetBytes([int]$entry.ExternalAttributes), 0)
            Assert-True (((($attrs -shr 16) -band 0xf000) -ne 0xa000)) "Windows ZIP contains a symlink: $($entry.FullName)"
            $target = [System.IO.Path]::GetFullPath((Join-Path $Root $entry.FullName))
            Assert-True ($target.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) "Windows ZIP entry escaped extraction root: $($entry.FullName)"
            $parent = Split-Path -Parent $target
            if (-not (Test-Path -LiteralPath $parent)) { $null = New-Item -ItemType Directory -Path $parent }
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $target, $false)
        }
    }
    finally { $zip.Dispose() }
    Assert-Sequence @(Get-PackageFiles $Root) $Expected 'Windows extracted allowlist'
}

function Invoke-Tar {
    param([string]$Tar, [string[]]$Arguments, [string]$Label)
    $output = @(& $Tar @Arguments 2>&1)
    if ($LASTEXITCODE -ne 0) { throw "$Label failed: $(@($output) -join ' ')" }
    @($output | ForEach-Object { [string]$_ })
}

function Expand-TarSafe {
    param([string]$Tar, [string]$Archive, [string]$Root, [string[]]$Expected)
    $names = @(Invoke-Tar $Tar @('-tzf', $Archive) 'Linux tar listing')
    Assert-SafeNames $names 'Linux tar'
    Assert-Sequence $names $Expected 'Linux tar allowlist and order'
    $details = @(Invoke-Tar $Tar @('-tvzf', $Archive) 'Linux tar type listing')
    Assert-True ($details.Count -eq $Expected.Count) 'Linux tar type listing count mismatch.'
    foreach ($line in $details) { Assert-True ($line.Length -gt 0 -and $line[0] -ceq '-') "Linux tar contains a non-file entry: $line" }
    $null = Invoke-Tar $Tar @('-xzf', $Archive, '-C', $Root) 'Linux tar extraction'
    Assert-Sequence @(Get-PackageFiles $Root) $Expected 'Linux extracted allowlist'
}

function Get-PropertyValue {
    param($Properties, [string]$Name, [string]$Label)
    $matches = @($Properties | Where-Object { [string]$_.name -ceq $Name })
    Assert-True ($matches.Count -eq 1) "$Label must contain exactly one $Name property."
    [string]$matches[0].value
}

function Assert-Binary {
    param([string]$Path, [string]$PackageId)
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($PackageId -ceq 'WINDOWS_LOCAL_RELEASE_ARCHIVE') {
        Assert-True ($bytes.Length -ge 512 -and $bytes[0] -eq 0x4d -and $bytes[1] -eq 0x5a) 'RBCSP.exe is not a valid PE executable.'
        $offset = [System.BitConverter]::ToInt32($bytes, 0x3c)
        Assert-True ($offset -ge 0x40 -and $offset + 26 -le $bytes.Length) 'RBCSP.exe has an invalid PE offset.'
        Assert-True ($bytes[$offset] -eq 0x50 -and $bytes[$offset + 1] -eq 0x45 -and [System.BitConverter]::ToUInt16($bytes, $offset + 4) -eq 0x8664) 'RBCSP.exe is not x86-64 PE.'
    }
    else {
        Assert-True ($bytes.Length -ge 64 -and $bytes[0] -eq 0x7f -and $bytes[1] -eq 0x45 -and $bytes[2] -eq 0x4c -and $bytes[3] -eq 0x46) 'bin/bcsp-server is not ELF.'
        Assert-True ($bytes[4] -eq 2 -and $bytes[5] -eq 1 -and [System.BitConverter]::ToUInt16($bytes, 18) -eq 62) 'bin/bcsp-server is not little-endian x86-64 ELF64.'
    }
}

function Assert-FrontendCapabilities {
    param($Capabilities, [string]$ExpectedTarget, [string]$PackageId)
    Assert-True (
        [int]$Capabilities.schemaVersion -eq 1 -and
        [string]$Capabilities.kind -ceq 'TARGET_BUILD_ALLOWLIST' -and
        [string]$Capabilities.target -ceq $ExpectedTarget -and
        [string]$Capabilities.readiness -ceq 'UI_INTEGRATION_COMPLETE'
    ) "$PackageId frontend capabilities identity mismatch."

    $validated = @{}
    foreach ($property in @('allowedCapabilities', 'allowedRoutes', 'allowedI18nCatalogs')) {
        $propertyMatch = @($Capabilities.PSObject.Properties | Where-Object { $_.Name -ceq $property })
        Assert-True ($propertyMatch.Count -eq 1 -and $propertyMatch[0].Value -is [System.Array]) "$PackageId frontend capabilities $property must be an array."
        $values = [string[]]@($propertyMatch[0].Value | ForEach-Object { [string]$_ })
        Assert-True ($values.Count -gt 0 -and @($values | Where-Object { $_.Length -eq 0 }).Count -eq 0) "$PackageId frontend capabilities $property must contain non-empty values."
        $unique = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
        foreach ($value in $values) {
            Assert-True ($unique.Add($value)) "$PackageId frontend capabilities $property contains a duplicate value: $value"
        }
        $validated[$property] = $values
    }
    $validated
}

function Assert-RequiredFrontendCapabilitySet {
    # The release gate on what the shipped frontends must declare. Kept as a
    # function of two plain capability arrays so the SelfTest parameter set
    # can execute it against fixtures without any archive; the Verify flow
    # calls it with the extracted packages' validated arrays. Returns the
    # shared-capability count for the release summary.
    param([string[]]$WindowsCapabilities, [string[]]$LinuxCapabilities)
    $requiredSharedCapabilities = @(
        'course-search',
        'course-section-details',
        'current-page-audio',
        'current-page-notification',
        'current-page-selection',
        'current-page-toast',
        'current-page-watch',
        'en-us-zh-cn',
        'filter-schema',
        'open-status',
        'refresh-diagnostics',
        'section-search'
    )
    foreach ($capability in $requiredSharedCapabilities) {
        Assert-True (@($WindowsCapabilities | Where-Object { $_ -ceq $capability }).Count -eq 1) "Windows frontend is missing shared capability: $capability"
        Assert-True (@($LinuxCapabilities | Where-Object { $_ -ceq $capability }).Count -eq 1) "Linux frontend is missing shared capability: $capability"
    }
    Assert-True (@($WindowsCapabilities | Where-Object { $_ -ceq 'windows-launcher-lifecycle' }).Count -eq 1) 'Windows frontend is missing windows-launcher-lifecycle.'
    Assert-True (@($LinuxCapabilities | Where-Object { $_ -ceq 'service-operations' }).Count -eq 1) 'Linux frontend is missing service-operations.'
    $requiredSharedCapabilities.Count
}

function Assert-CleanPayload {
    param([string]$Root, [string[]]$Files, [string]$PackageId, [string]$Binary)
    $contentPatterns = @(
        @{ Name = 'a private-key marker'; Regex = '(?i)-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----' },
        @{ Name = 'a credential-shaped token'; Regex = '(?i)(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9_-]{20,})' },
        @{ Name = 'credentials in a URL'; Regex = '(?i)\b(?:https?|postgres(?:ql)?|mysql)://[^\s\x00/@:]+:[^\s\x00/@]+@' },
        @{ Name = 'an absolute Windows user path'; Regex = '(?i)[A-Z]:[\\/](?:Users|Documents and Settings)[\\/](?!<|%)[^\\/\x00\r\n]+[\\/]' },
        @{ Name = 'an absolute Unix user path'; Regex = '(?i)/(?:home|Users)/(?!<|\$|\{)[A-Za-z0-9._-]+/|/root/' },
        @{ Name = 'a forbidden real-data marker'; Regex = 'RAW_REAL_CATALOG_OR_OPEN_DATA|PREINSTALLED_CATALOG_OR_OPEN_ROWS|PRECREATED_RUNTIME_DATABASE' },
        @{ Name = 'a serialized real-course record'; Regex = '(?i)"subject"\s*:\s*"[A-Z0-9]{2,6}"|"courseNumber"\s*:\s*"?\d{3}[A-Z]?"?|"index"\s*:\s*"?\d{5}"?' }
    )
    foreach ($file in $Files) {
        Assert-True ($file -notmatch '(?i)(^|/)(?:data|share|tests|\.secrets)(/|$)|\.(?:db|sqlite|sqlite3)$|-(?:journal|shm|wal)$|chat-log-') "$PackageId contains a forbidden file: $file"
        $bytes = [System.IO.File]::ReadAllBytes((Join-Path $Root $file))
        $text = [System.Text.Encoding]::ASCII.GetString($bytes)
        foreach ($check in $contentPatterns) { Assert-True ($text -notmatch $check.Regex) "$PackageId contains $($check.Name) in $file."
        }
        if ($bytes.Length -ge 4) {
            $pe = $bytes[0] -eq 0x4d -and $bytes[1] -eq 0x5a
            $elf = $bytes[0] -eq 0x7f -and $bytes[1] -eq 0x45 -and $bytes[2] -eq 0x4c -and $bytes[3] -eq 0x46
            if ($PackageId -ceq 'WINDOWS_LOCAL_RELEASE_ARCHIVE') {
                Assert-True (-not $elf -and (-not $pe -or $file -ceq $Binary)) "Windows package contains a platform-crossing executable: $file"
            }
            else { Assert-True (-not $pe -and (-not $elf -or $file -ceq $Binary)) "Linux package contains a platform-crossing executable: $file" }
        }
    }
}

function Test-Package {
    param($Package, $Inputs, [string]$Archive, [string]$Root, [string[]]$Files, [string]$Binary, [string]$RootName)
    $version = [string]$Inputs.releaseVersion
    Assert-True ((Get-Content -LiteralPath (Join-Path $Root 'VERSION') -Raw -Encoding UTF8) -ceq "$version`n") "$($Package.id) VERSION mismatch."
    Assert-True ((Get-Sha256 (Join-Path $Root 'LICENSE')) -ceq (Get-Sha256 (Join-Path $RepositoryRoot 'LICENSE'))) "$($Package.id) LICENSE differs from the repository ISC license."

    $manifest = Read-Json (Join-Path $Root 'MANIFEST.json')
    Assert-True ([int]$manifest.schemaVersion -eq 1 -and [string]$manifest.packageId -ceq [string]$Package.id -and [string]$manifest.archiveName -ceq [string]$Package.archiveName) "$($Package.id) manifest identity mismatch."
    Assert-True ([string]$manifest.version -ceq $version -and [string]$manifest.target -ceq [string]$Package.target) "$($Package.id) manifest version or target mismatch."
    Assert-True ([string]$manifest.sourceCommit -cmatch '^[0-9a-f]{40}(?:[0-9a-f]{24})?$' -and [long]$manifest.sourceDateEpoch -ge 315532800) "$($Package.id) manifest source identity is invalid."
    Assert-Sequence @($manifest.excludedFiles | ForEach-Object { [string]$_ }) @('MANIFEST.json', 'SHA256SUMS') "$($Package.id) manifest exclusions"
    $manifestFiles = @($Files | Where-Object { $_ -cne 'MANIFEST.json' -and $_ -cne 'SHA256SUMS' })
    Assert-Sequence @($manifest.files | ForEach-Object { [string]$_.path }) $manifestFiles "$($Package.id) manifest files"
    Assert-True ([int]$manifest.fileCount -eq $manifestFiles.Count) "$($Package.id) manifest fileCount mismatch."
    foreach ($entry in @($manifest.files)) {
        $path = Join-Path $Root ([string]$entry.path)
        Assert-True ([long]$entry.sizeBytes -eq (Get-Item -LiteralPath $path).Length -and [string]$entry.sha256 -ceq (Get-Sha256 $path)) "$($Package.id) manifest mismatch for $($entry.path)."
    }

    $sumText = Get-Content -LiteralPath (Join-Path $Root 'SHA256SUMS') -Raw -Encoding UTF8
    Assert-True (-not $sumText.Contains("`r") -and $sumText.EndsWith("`n")) "$($Package.id) SHA256SUMS line endings are invalid."
    $sumFiles = @($Files | Where-Object { $_ -cne 'SHA256SUMS' })
    $sumLines = @($sumText.TrimEnd("`n").Split("`n"))
    Assert-True ($sumLines.Count -eq $sumFiles.Count) "$($Package.id) SHA256SUMS count mismatch."
    $seenSums = New-Object System.Collections.Generic.List[string]
    foreach ($line in $sumLines) {
        Assert-True ($line -cmatch '^(?<hash>[0-9a-f]{64})  (?<path>.+)$') "$($Package.id) has an invalid SHA256SUMS line."
        $seenSums.Add([string]$Matches.path)
        Assert-True ([string]$Matches.hash -ceq (Get-Sha256 (Join-Path $Root $Matches.path))) "$($Package.id) SHA256SUMS mismatch for $($Matches.path)."
    }
    Assert-Sequence @($seenSums) $sumFiles "$($Package.id) SHA256SUMS files"

    $provenance = Read-Json (Join-Path $Root 'BUILD-PROVENANCE.json')
    Assert-True ([int]$provenance.schemaVersion -eq 1 -and [string]$provenance.packageId -ceq [string]$Package.id -and [string]$provenance.archiveName -ceq [string]$Package.archiveName) "$($Package.id) provenance identity mismatch."
    Assert-True ([string]$provenance.version -ceq $version -and [string]$provenance.source.commit -ceq [string]$manifest.sourceCommit -and [long]$provenance.source.dateEpoch -eq [long]$manifest.sourceDateEpoch) "$($Package.id) provenance source mismatch."
    Assert-True ([string]$provenance.build.target -ceq [string]$Package.target -and [string]$provenance.build.frontendTarget -ceq [string]$Package.frontendTarget -and [bool]$provenance.build.cargoLocked -and [bool]$provenance.build.embeddedWebUi) "$($Package.id) provenance build mismatch."
    Assert-True ([string]$provenance.tools.rustc -ceq [string]$Inputs.lockedToolchain.rust -and [string]$provenance.tools.cargo -ceq [string]$Inputs.lockedToolchain.rust) "$($Package.id) Rust toolchain mismatch."
    Assert-True ([string]$provenance.tools.node -ceq [string]$Inputs.lockedToolchain.node -and [string]$provenance.tools.npm -ceq [string]$Inputs.lockedToolchain.npm) "$($Package.id) frontend toolchain mismatch."
    $inputPaths = @('Cargo.lock', 'frontend/package-lock.json', 'packaging/about.toml', 'packaging/generate-release-metadata.mjs', 'packaging/licenses/MIT.txt', 'packaging/release-inputs.json')
    Assert-Sequence @($provenance.inputs | ForEach-Object { [string]$_.path }) $inputPaths "$($Package.id) provenance inputs"
    foreach ($input in @($provenance.inputs)) { Assert-True ([string]$input.sha256 -cmatch '^[0-9a-f]{64}$') "$($Package.id) has an invalid provenance input hash." }
    $binaryPath = Join-Path $Root $Binary
    Assert-True ([string]$provenance.artifact.path -ceq $Binary -and [long]$provenance.artifact.sizeBytes -eq (Get-Item -LiteralPath $binaryPath).Length -and [string]$provenance.artifact.sha256 -ceq (Get-Sha256 $binaryPath)) "$($Package.id) provenance artifact mismatch."

    $frontendCapabilities = Read-Json (Join-Path $Root 'FRONTEND-CAPABILITIES.json')
    $validatedFrontendCapabilities = Assert-FrontendCapabilities $frontendCapabilities ([string]$Package.frontendTarget) ([string]$Package.id)

    $sbom = Read-Json (Join-Path $Root 'SBOM.cdx.json')
    Assert-True ([string]$sbom.bomFormat -ceq 'CycloneDX' -and [string]$sbom.specVersion -ceq '1.6' -and [int]$sbom.version -eq 1) "$($Package.id) SBOM format mismatch."
    Assert-True (@($sbom.components).Count -gt 0 -and @($sbom.dependencies).Count -gt 0) "$($Package.id) SBOM graph is empty."
    $rootRef = "pkg:cargo/$RootName@$version"
    Assert-True ([string]$sbom.metadata.component.'bom-ref' -ceq $rootRef -and [string]$sbom.metadata.component.purl -ceq $rootRef) "$($Package.id) SBOM root mismatch."
    Assert-True ((Get-PropertyValue $sbom.metadata.component.properties 'rbcsp:packageId' 'SBOM root') -ceq [string]$Package.id) "$($Package.id) SBOM packageId mismatch."
    Assert-True ((Get-PropertyValue $sbom.metadata.component.properties 'rbcsp:target' 'SBOM root') -ceq [string]$Package.target) "$($Package.id) SBOM target mismatch."
    Assert-True ((Get-PropertyValue $sbom.metadata.component.properties 'rbcsp:frontendTarget' 'SBOM root') -ceq [string]$Package.frontendTarget) "$($Package.id) SBOM frontend target mismatch."
    Assert-True ((Get-PropertyValue $sbom.metadata.component.properties 'rbcsp:sourceCommit' 'SBOM root') -ceq [string]$manifest.sourceCommit) "$($Package.id) SBOM source mismatch."
    Assert-True (@($sbom.metadata.component.hashes | Where-Object { [string]$_.alg -ceq 'SHA-256' -and [string]$_.content -ceq (Get-Sha256 $binaryPath) }).Count -eq 1) "$($Package.id) SBOM binary hash mismatch."
    $components = @{}
    $frontendRefs = New-Object System.Collections.Generic.List[string]
    foreach ($component in @($sbom.components)) {
        $reference = [string]$component.'bom-ref'
        Assert-True ($reference.Length -gt 0 -and -not $components.ContainsKey($reference)) "$($Package.id) SBOM contains a duplicate component."
        $components[$reference] = $component
        if (@($component.properties | Where-Object { [string]$_.name -ceq 'rbcsp:ecosystem' -and [string]$_.value -ceq 'npm' }).Count -eq 1) { $frontendRefs.Add($reference) }
    }
    Assert-True ($frontendRefs.Count -gt 1) "$($Package.id) SBOM does not contain the embedded frontend closure."

    $notices = Get-Content -LiteralPath (Join-Path $Root 'THIRD-PARTY-NOTICES.txt') -Raw -Encoding UTF8
    Assert-True ($notices.Contains("Package: $($Package.archiveName)") -and $notices.Contains("Version: $version") -and $notices.Contains("Target: $($Package.target)") -and $notices.Contains('License texts')) "$($Package.id) third-party notices are incomplete."
    Assert-CleanPayload $Root $Files ([string]$Package.id) $Binary
    Assert-Binary $binaryPath ([string]$Package.id)

    [pscustomobject]@{
        Package = $Package; Manifest = $manifest; Provenance = $provenance; Components = $components
        FrontendCapabilities = $validatedFrontendCapabilities
        FrontendRefs = [string[]](Sort-Ordinal @($frontendRefs)); ArchiveHash = Get-Sha256 $Archive
        FileCount = $Files.Count; ComponentCount = $components.Count
    }
}

if ($PSCmdlet.ParameterSetName -ceq 'SelfTest') {
    try {
        # The fixture list is DELIBERATELY a second copy of the required set:
        # if anyone deletes a slug from the gate, the negative fixture below
        # sails through it and this self-test fails -- which is the point.
        $sharedFixture = @(
            'course-search',
            'course-section-details',
            'current-page-audio',
            'current-page-notification',
            'current-page-selection',
            'current-page-toast',
            'current-page-watch',
            'en-us-zh-cn',
            'filter-schema',
            'open-status',
            'refresh-diagnostics',
            'section-search'
        )
        $windowsFixture = [string[]]@($sharedFixture + 'windows-launcher-lifecycle')
        $linuxFixture = [string[]]@($sharedFixture + 'service-operations')

        # Positive: a compliant capability set passes the gate.
        $null = Assert-RequiredFrontendCapabilitySet $windowsFixture $linuxFixture

        # Negative, once per shared slug: the fixture minus that one slug must
        # be refused, and refused FOR that slug. This is what makes the copy
        # discriminating for the whole set, current-page-notification
        # included, rather than only for whichever slug a reviewer thought of.
        foreach ($slug in $sharedFixture) {
            $without = [string[]]@($sharedFixture | Where-Object { $_ -cne $slug })
            $refused = $null
            try {
                $null = Assert-RequiredFrontendCapabilitySet ([string[]]@($without + 'windows-launcher-lifecycle')) ([string[]]@($without + 'service-operations'))
            }
            catch { $refused = $_.Exception.Message }
            Assert-True ($null -ne $refused) "A capability set without $slug was accepted."
            Assert-True ($refused.Contains($slug)) "The refusal did not name the missing slug ${slug}: $refused"
        }

        # Negative: the per-platform slugs are required too.
        $refusedPlatform = $null
        try { $null = Assert-RequiredFrontendCapabilitySet $windowsFixture ([string[]]@($sharedFixture)) }
        catch { $refusedPlatform = $_.Exception.Message }
        Assert-True ($null -ne $refusedPlatform) 'A Linux capability set without service-operations was accepted.'
        $refusedWindows = $null
        try { $null = Assert-RequiredFrontendCapabilitySet ([string[]]@($sharedFixture)) $linuxFixture }
        catch { $refusedWindows = $_.Exception.Message }
        Assert-True ($null -ne $refusedWindows) 'A Windows capability set without windows-launcher-lifecycle was accepted.'

        [pscustomobject]@{
            state = 'PASS'; mode = 'SELF_TEST'; checkedSharedCapabilities = $sharedFixture.Count
        } | ConvertTo-Json -Compress
        exit 0
    }
    catch {
        [System.Console]::Error.WriteLine("release-set-selftest: $($_.Exception.Message)")
        exit 1
    }
}

try {
    Assert-True (Test-Path -LiteralPath $InputsPath -PathType Leaf) "Missing $InputsPath"
    $inputs = Read-Json $InputsPath
    Assert-True ([int]$inputs.schemaVersion -eq 1 -and [int]$inputs.packageCount -eq 2 -and @($inputs.packages).Count -eq 2) 'release-inputs.json must define exactly two packages.'
    $definitions = @{
        WINDOWS_LOCAL_RELEASE_ARCHIVE = @('x86_64-pc-windows-msvc', 'apps/bcsp-local', 'local', 'data/rbcsp.sqlite', 12, 'RBCSP.exe', 'bcsp-local')
        LINUX_PUBLIC_DEPLOYMENT_PACKAGE = @('x86_64-unknown-linux-gnu', 'apps/bcsp-server', 'public', '/var/lib/bcsp/rbcsp.sqlite', 22, 'bin/bcsp-server', 'bcsp-server')
    }
    $packages = @{}
    foreach ($package in @($inputs.packages)) {
        $id = [string]$package.id
        Assert-True ($definitions.ContainsKey($id) -and -not $packages.ContainsKey($id)) "Unexpected or duplicate package id: $id"
        $definition = $definitions[$id]
        Assert-True ([string]$package.target -ceq $definition[0] -and [string]$package.binary -ceq $definition[1] -and [string]$package.frontendTarget -ceq $definition[2] -and [string]$package.runtimeDataPath -ceq $definition[3]) "$id definition mismatch."
        $allowlist = [string[]]@($package.allowlist | ForEach-Object { [string]$_ })
        Assert-True ($allowlist.Count -eq $definition[4]) "$id allowlist count mismatch."
        Assert-SafeNames $allowlist "$id allowlist"
        Assert-Sequence $allowlist @(Sort-Ordinal $allowlist) "$id allowlist order"
        $packages[$id] = $package
    }

    $ReleaseDirectory = [System.IO.Path]::GetFullPath($ReleaseDirectory)
    Assert-True (Test-Path -LiteralPath $ReleaseDirectory -PathType Container) "Release directory does not exist: $ReleaseDirectory"
    $entries = @(Get-ChildItem -LiteralPath $ReleaseDirectory -Force)
    $expectedNames = @(Sort-Ordinal @($inputs.packages | ForEach-Object { [string]$_.archiveName }))
    Assert-Sequence @(Sort-Ordinal @($entries | ForEach-Object { [string]$_.Name })) $expectedNames 'Release directory entries'
    foreach ($entry in $entries) { Assert-True (-not $entry.PSIsContainer -and ($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) "Release entry is not a regular file: $($entry.Name)" }

    $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
    if (-not $tar) { $tar = Get-Command tar -ErrorAction SilentlyContinue }
    Assert-True ($null -ne $tar) 'tar is required to inspect the Linux archive.'
    $tempParent = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
    $temp = [System.IO.Path]::GetFullPath((Join-Path $tempParent ('rbcsp-release-set-' + [System.Guid]::NewGuid().ToString('N'))))
    Assert-True ($temp.StartsWith($tempParent, [System.StringComparison]::OrdinalIgnoreCase)) 'Temporary extraction path escaped the system temp directory.'
    $windowsRoot = Join-Path $temp 'windows'; $linuxRoot = Join-Path $temp 'linux'
    $null = New-Item -ItemType Directory -Path $windowsRoot; $null = New-Item -ItemType Directory -Path $linuxRoot
    try {
        $windowsPackage = $packages['WINDOWS_LOCAL_RELEASE_ARCHIVE']; $linuxPackage = $packages['LINUX_PUBLIC_DEPLOYMENT_PACKAGE']
        $windowsFiles = [string[]]@($windowsPackage.allowlist); $linuxFiles = [string[]]@($linuxPackage.allowlist)
        $windowsArchive = Join-Path $ReleaseDirectory ([string]$windowsPackage.archiveName)
        $linuxArchive = Join-Path $ReleaseDirectory ([string]$linuxPackage.archiveName)
        Expand-ZipSafe $windowsArchive $windowsRoot $windowsFiles
        Expand-TarSafe ([string]$tar.Source) $linuxArchive $linuxRoot $linuxFiles
        $windows = Test-Package $windowsPackage $inputs $windowsArchive $windowsRoot $windowsFiles 'RBCSP.exe' 'bcsp-local'
        $linux = Test-Package $linuxPackage $inputs $linuxArchive $linuxRoot $linuxFiles 'bin/bcsp-server' 'bcsp-server'

        Assert-True ([string]$windows.Manifest.version -ceq [string]$linux.Manifest.version -and [string]$windows.Manifest.sourceCommit -ceq [string]$linux.Manifest.sourceCommit -and [long]$windows.Manifest.sourceDateEpoch -eq [long]$linux.Manifest.sourceDateEpoch) 'Packages differ in version, source commit, or source epoch.'
        Assert-True ([string]$windows.Manifest.sourceCommit -ceq $SourceCommit -and [long]$windows.Manifest.sourceDateEpoch -eq $SourceDateEpoch) 'Packages do not match the requested source commit and source epoch.'
        Assert-True ((ConvertTo-Json $windows.Provenance.tools -Compress) -ceq (ConvertTo-Json $linux.Provenance.tools -Compress)) 'Packages differ in toolchain provenance.'
        for ($index = 0; $index -lt @($windows.Provenance.inputs).Count; $index += 1) {
            Assert-True ([string]$windows.Provenance.inputs[$index].sha256 -ceq [string]$linux.Provenance.inputs[$index].sha256) "Packages differ in provenance input hash for $($windows.Provenance.inputs[$index].path)."
        }
        $common = @(Sort-Ordinal @($windows.Components.Keys | Where-Object { $linux.Components.ContainsKey($_) }))
        Assert-True ($common.Count -gt 0) 'SBOMs have no shared components.'
        foreach ($reference in $common) { Assert-True ((ConvertTo-Json $windows.Components[$reference] -Depth 20 -Compress) -ceq (ConvertTo-Json $linux.Components[$reference] -Depth 20 -Compress)) "Shared SBOM component differs: $reference" }
        Assert-Sequence $windows.FrontendRefs $linux.FrontendRefs 'Embedded frontend components'

        $sharedFrontendCapabilityCount = Assert-RequiredFrontendCapabilitySet ([string[]]@($windows.FrontendCapabilities.allowedCapabilities)) ([string[]]@($linux.FrontendCapabilities.allowedCapabilities))

        [pscustomobject]@{
            state = 'PASS'; version = [string]$inputs.releaseVersion; sourceCommit = [string]$windows.Manifest.sourceCommit
            sourceDateEpoch = [long]$windows.Manifest.sourceDateEpoch
            packages = @(
                [pscustomobject]@{ id = [string]$windowsPackage.id; archive = [string]$windowsPackage.archiveName; sha256 = $windows.ArchiveHash; files = $windows.FileCount }
                [pscustomobject]@{ id = [string]$linuxPackage.id; archive = [string]$linuxPackage.archiveName; sha256 = $linux.ArchiveHash; files = $linux.FileCount }
            )
            sharedComponents = $common.Count; frontendComponents = $windows.FrontendRefs.Count
            sharedFrontendCapabilities = $sharedFrontendCapabilityCount
        } | ConvertTo-Json -Depth 5 -Compress
    }
    finally { if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Recurse -Force } }
}
catch {
    [System.Console]::Error.WriteLine("release-set-verify: $($_.Exception.Message)")
    exit 1
}
