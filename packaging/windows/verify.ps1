[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath,

    [Parameter(Mandatory = $true)]
    [string]$SourceCommit,

    [Parameter(Mandatory = $true)]
    [long]$SourceDateEpoch,

    # The deterministic PUBLISHED + Gate-pass fixture seeder, built from this
    # repository as a test artifact and never part of the archive.
    #
    # Mandatory on purpose. Without it the restart lifetime can only prove
    # that stored intent SURVIVES, which is a strictly weaker claim than the
    # one the release gate is about -- that a real candidate puts a real
    # watch behind it again -- and an optional parameter is exactly how a gate
    # quietly stops being run.
    [Parameter(Mandatory = $true)]
    [string]$FixtureSeederPath,

    [string]$DumpBinPath,

    [string]$BrowserSmokeScript,

    [string]$PlaywrightRoot,

    [string]$NodePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$PackageId = 'WINDOWS_LOCAL_RELEASE_ARCHIVE'
if ($SourceCommit -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') {
    throw 'SourceCommit must be a full lowercase hexadecimal commit id.'
}
if ($SourceDateEpoch -lt 0) {
    throw 'SourceDateEpoch must be a non-negative integer.'
}
$RepositoryRoot = [System.IO.Path]::GetFullPath(
    (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)
$ReleaseInputsPath = Join-Path $RepositoryRoot 'packaging\release-inputs.json'
$ArchivePath = [System.IO.Path]::GetFullPath($ArchivePath)
$releaseInputs = Get-Content -LiteralPath $ReleaseInputsPath -Raw | ConvertFrom-Json
$packageMatches = @($releaseInputs.packages | Where-Object { $_.id -eq $PackageId })
if ($packageMatches.Count -ne 1) {
    throw "release-inputs.json must define exactly one $PackageId package."
}
$package = $packageMatches[0]
$expectedFiles = @($package.allowlist | ForEach-Object { [string]$_ } | Sort-Object)

function Assert-Condition {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Get-LowerSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-DumpBin {
    if ($DumpBinPath) {
        $candidate = [System.IO.Path]::GetFullPath($DumpBinPath)
        Assert-Condition (Test-Path -LiteralPath $candidate -PathType Leaf) "dumpbin.exe was not found at $candidate"
        return $candidate
    }

    $command = Get-Command dumpbin.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command) {
        return $command.Source
    }

    $roots = @(
        'C:\Software\VSBuildTools\VC\Tools\MSVC',
        'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC',
        'C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC',
        'C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC',
        'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC'
    )
    foreach ($root in $roots) {
        if (Test-Path -LiteralPath $root -PathType Container) {
            $candidate = Get-ChildItem -LiteralPath $root -Recurse -Filter dumpbin.exe -File |
                Where-Object { $_.FullName -match '\\bin\\Hostx64\\x64\\dumpbin\.exe$' } |
                Sort-Object FullName -Descending |
                Select-Object -First 1
            if ($candidate) {
                return $candidate.FullName
            }
        }
    }
    throw 'dumpbin.exe from a Visual Studio 2022 x64 C++ toolchain is required.'
}

function Read-Bootstrap {
    param([Parameter(Mandatory = $true)][string]$Origin)

    return Invoke-RestMethod -UseBasicParsing -Method Get -Uri ($Origin + 'api/v1/local/bootstrap') -TimeoutSec 10
}

function Read-DesiredWatch {
    param([Parameter(Mandatory = $true)][string]$Origin)

    return Invoke-RestMethod -UseBasicParsing -Method Get `
        -Uri ($Origin + 'api/v1/local/desired-watch') -TimeoutSec 10
}

function Write-DesiredWatch {
    param(
        [Parameter(Mandatory = $true)][string]$Origin,
        [Parameter(Mandatory = $true)][hashtable]$Headers,
        [Parameter(Mandatory = $true)]$Section,
        [Parameter(Mandatory = $true)][long]$AuthorityGeneration,
        [Parameter(Mandatory = $true)][long]$BasedOnRevision,
        $Policy
    )

    $payload = [ordered]@{
        contractVersion = 1
        section = $Section
        policy = $Policy
        basedOnRevision = $BasedOnRevision
        authorityGeneration = $AuthorityGeneration
        mutationId = [guid]::NewGuid().ToString()
    }
    $body = [ordered]@{ protocolVersion = 1; payload = $payload } |
        ConvertTo-Json -Depth 8 -Compress
    return Invoke-RestMethod -UseBasicParsing -Method Put `
        -Uri ($Origin + 'api/v1/local/desired-watch') -Headers $Headers `
        -ContentType 'application/json' -Body $body -TimeoutSec 10
}

function Invoke-FixtureSeeder {
    param(
        [Parameter(Mandatory = $true)][string]$CandidateRoot,
        [Parameter(Mandatory = $true)][string]$Term
    )

    Write-Host '==> Seeding the deterministic PUBLISHED + Gate-pass fixture'
    $executable = Join-Path $CandidateRoot 'RBCSP.exe'
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $FixtureSeederPath --executable $executable --term $Term 2>&1 |
            ForEach-Object { Write-Host ([string]$_) }
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    Assert-Condition ($exitCode -eq 0) "The desired-watch fixture seeder failed with exit code $exitCode."
    $sidecars = @(Get-ChildItem -LiteralPath (Join-Path $CandidateRoot 'data') -File |
        Where-Object { $_.Name -match '(?i)(-wal|-shm)$' })
    foreach ($sidecar in $sidecars) {
        Remove-Item -LiteralPath $sidecar.FullName -Force
    }
    $extra = @(Get-ChildItem -LiteralPath $CandidateRoot -File | ForEach-Object { $_.Name } | Sort-Object)
    Assert-Condition (
        ($extra -join "`n") -eq ($expectedFiles -join "`n")
    ) 'The fixture seeder left a file in the package root.'
}

function Open-WatchSocket {
    param(
        [Parameter(Mandatory = $true)][string]$Origin,
        [Parameter(Mandatory = $true)][string]$SessionNonce
    )

    $trimmed = $Origin.TrimEnd('/')
    $socketUri = [Uri]($trimmed.Replace('http://', 'ws://') + '/api/v1/watch?session=' + $SessionNonce)
    $socket = New-Object System.Net.WebSockets.ClientWebSocket
    $socket.Options.AddSubProtocol('bcsp.v1')
    $socket.Options.SetRequestHeader('Origin', $trimmed)
    $cancellation = New-Object System.Threading.CancellationTokenSource
    $connect = $socket.ConnectAsync($socketUri, $cancellation.Token)
    Assert-Condition ($connect.Wait(15000)) 'The watch WebSocket did not complete its handshake within 15 seconds.'
    Assert-Condition (-not $connect.IsFaulted) "The watch WebSocket handshake failed: $($connect.Exception)"
    Assert-Condition (
        $socket.State -eq [System.Net.WebSockets.WebSocketState]::Open
    ) "The watch WebSocket is $($socket.State), not Open."
    return [pscustomobject]@{ Socket = $socket; Cancellation = $cancellation }
}

function Close-WatchSocket {
    param($Attachment)

    if (-not $Attachment) {
        return
    }
    try {
        $Attachment.Socket.Abort()
        $Attachment.Socket.Dispose()
        $Attachment.Cancellation.Dispose()
    }
    catch {
        # Preserve the original verification failure.
    }
}

function Wait-DesiredWatchMaterialized {
    param(
        [Parameter(Mandatory = $true)][string]$Origin,
        [Parameter(Mandatory = $true)]$Section
    )

    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    $last = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        $last = Read-DesiredWatch $Origin
        $entries = @($last.data.entries | Where-Object {
            [string]$_.section.term -ceq [string]$Section.term -and
            [string]$_.section.campus -ceq [string]$Section.campus -and
            [string]$_.section.index -ceq [string]$Section.index
        })
        if ($entries.Count -eq 1 -and $null -ne $entries[0].materialized) {
            return [pscustomobject]@{ State = $last; Entry = $entries[0] }
        }
        Start-Sleep -Milliseconds 200
    }
    throw "The restored desired watch never materialized: $($last | ConvertTo-Json -Depth 8 -Compress)"
}

function Read-ServiceStatus {
    param([Parameter(Mandatory = $true)][string]$Origin)

    return Invoke-RestMethod -UseBasicParsing -Method Get -Uri ($Origin + 'api/v1/service/status') -TimeoutSec 10
}

function Read-SharedText {
    param([Parameter(Mandatory = $true)][string]$Path)

    $stream = New-Object System.IO.FileStream(
        $Path,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::ReadWrite
    )
    try {
        $reader = New-Object System.IO.StreamReader($stream, [System.Text.Encoding]::UTF8, $true)
        try {
            return $reader.ReadToEnd()
        }
        finally {
            $reader.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Stop-TestProcessTree {
    param($Process)

    if (-not $Process) {
        return
    }
    try {
        $Process.Refresh()
        if (-not $Process.HasExited) {
            $previousPreference = $ErrorActionPreference
            try {
                $ErrorActionPreference = 'Continue'
                & taskkill.exe /PID $Process.Id /T /F *> $null
            }
            finally {
                $ErrorActionPreference = $previousPreference
            }
        }
    }
    catch {
        # Preserve the original verification failure.
    }
}

function Start-Candidate {
    param(
        [Parameter(Mandatory = $true)][string]$CandidateRoot,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory,
        [Parameter(Mandatory = $true)][string]$LogLabel,
        [Parameter(Mandatory = $true)][string]$LogRoot,
        [switch]$UseLauncher
    )

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    if ($UseLauncher) {
        $launcher = Join-Path $CandidateRoot 'Start-RBCSP.bat'
        $startInfo.FileName = $env:ComSpec
        $startInfo.Arguments = "/d /s /c `"`"$launcher`"`""
    }
    else {
        $startInfo.FileName = Join-Path $CandidateRoot 'RBCSP.exe'
        $startInfo.Arguments = ''
    }
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
    $startInfo.EnvironmentVariables['BCSP_CI_NO_RUTGERS'] = '1'
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    Assert-Condition ($process.Start()) 'Windows could not start the RBCSP candidate process.'

    $lockPath = Join-Path $CandidateRoot 'data\rbcsp.instance.lock'
    $deadline = [DateTime]::UtcNow.AddSeconds(45)
    $origin = $null
    while ([DateTime]::UtcNow -lt $deadline) {
        $process.Refresh()
        if ($process.HasExited) {
            throw "RBCSP exited before publishing its local URL (exit $($process.ExitCode))."
        }
        if (Test-Path -LiteralPath $lockPath -PathType Leaf) {
            try {
                $lockText = Read-SharedText $lockPath
                if ($lockText -match '^http://127\.0\.0\.1:(?<port>[1-9][0-9]{0,4})/\n$') {
                    $port = [int]$Matches.port
                    if ($port -le 65535) {
                        $origin = $lockText.TrimEnd("`n")
                        $null = Read-Bootstrap $origin
                        break
                    }
                }
            }
            catch {
                # The server may publish the lock just before the first request is accepted.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    if (-not $origin) {
        Stop-TestProcessTree $process
        throw 'RBCSP did not publish a healthy canonical loopback URL within 45 seconds.'
    }

    return [pscustomobject]@{
        Process = $process
        Origin = $origin
        LockPath = $lockPath
    }
}

function Stop-CandidateGracefully {
    param(
        [Parameter(Mandatory = $true)]$Run,
        [Parameter(Mandatory = $true)][string]$SessionNonce
    )

    $headers = @{
        Origin = $Run.Origin.TrimEnd('/')
        'x-bcsp-session' = $SessionNonce
    }
    $response = Invoke-WebRequest -UseBasicParsing -Method Post `
        -Uri ($Run.Origin + 'api/v1/local/exit') -Headers $headers `
        -ContentType 'application/json' -Body '' -TimeoutSec 10
    Assert-Condition ($response.StatusCode -eq 204) "Local exit returned HTTP $($response.StatusCode), not 204."
    Assert-Condition ($Run.Process.WaitForExit(15000)) 'RBCSP did not exit within 15 seconds after the local exit request.'
    $Run.Process.WaitForExit()
    $Run.Process.Refresh()
    $exitCode = $Run.Process.ExitCode
    Assert-Condition ($exitCode -eq 0) "RBCSP exited with code $exitCode."

    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    while ((Test-Path -LiteralPath $Run.LockPath) -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
    }
    Assert-Condition (-not (Test-Path -LiteralPath $Run.LockPath)) 'The local instance lock remained after graceful exit.'
}

function Stop-CandidateAfterFailure {
    param($Run)

    if (-not $Run) {
        return
    }
    Stop-TestProcessTree $Run.Process
}

function Assert-EmptyPersonalState {
    param(
        [Parameter(Mandatory = $true)]$Bootstrap,
        [Parameter(Mandatory = $true)][string]$Label
    )

    Assert-Condition (@($Bootstrap.data.state.savedViews).Count -eq 0) "$Label Saved views are not empty."
    Assert-Condition (@($Bootstrap.data.state.selectedSections).Count -eq 0) "$Label selected Sections are not empty."
    Assert-Condition (@($Bootstrap.data.state.desiredWatches).Count -eq 0) "$Label desired watches are not empty."
    Assert-Condition (@($Bootstrap.data.state.episodeHistory.items).Count -eq 0) "$Label local history is not empty."
    Assert-Condition ($null -eq $Bootstrap.data.state.currentFilters.value) "$Label current filters are not empty."
    Assert-Condition ([int]$Bootstrap.data.state.activeWatchCount -eq 0) "$Label active watch count is not zero."
}

function Invoke-BrowserSmoke {
    param(
        [Parameter(Mandatory = $true)][string]$BaseUrl,
        [Parameter(Mandatory = $true)][string]$Script,
        [Parameter(Mandatory = $true)][string]$Playwright,
        [Parameter(Mandatory = $true)][string]$Node
    )

    Write-Host '==> Running the shared local candidate browser acceptance'
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $Node $Script `
            --mode local `
            --base-url $BaseUrl.TrimEnd('/') `
            --playwright-root $Playwright 2>&1 |
            ForEach-Object { Write-Host ([string]$_) }
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    Assert-Condition ($exitCode -eq 0) "Shared local candidate browser acceptance failed with exit code $exitCode."
}

Assert-Condition (Test-Path -LiteralPath $ArchivePath -PathType Leaf) "Archive was not found: $ArchivePath"
Assert-Condition ([System.IO.Path]::GetFileName($ArchivePath) -eq $package.archiveName) "Archive name must be $($package.archiveName)."

$browserOptions = @($BrowserSmokeScript, $PlaywrightRoot, $NodePath)
$configuredBrowserOptions = @($browserOptions | Where-Object {
    -not [string]::IsNullOrWhiteSpace([string]$_)
})
Assert-Condition (
    $configuredBrowserOptions.Count -eq 0 -or $configuredBrowserOptions.Count -eq 3
) 'BrowserSmokeScript, PlaywrightRoot, and NodePath must be supplied together or omitted together.'
$runBrowserSmoke = $configuredBrowserOptions.Count -eq 3
if ($runBrowserSmoke) {
    $BrowserSmokeScript = [System.IO.Path]::GetFullPath($BrowserSmokeScript)
    $PlaywrightRoot = [System.IO.Path]::GetFullPath($PlaywrightRoot)
    $NodePath = [System.IO.Path]::GetFullPath($NodePath)
    Assert-Condition (Test-Path -LiteralPath $BrowserSmokeScript -PathType Leaf) "Browser smoke script was not found: $BrowserSmokeScript"
    Assert-Condition (Test-Path -LiteralPath $PlaywrightRoot -PathType Container) "Playwright root was not found: $PlaywrightRoot"
    Assert-Condition (Test-Path -LiteralPath $NodePath -PathType Leaf) "Node executable was not found: $NodePath"
}

$FixtureSeederPath = [System.IO.Path]::GetFullPath($FixtureSeederPath)
Assert-Condition (
    Test-Path -LiteralPath $FixtureSeederPath -PathType Leaf
) "The desired-watch fixture seeder was not found: $FixtureSeederPath"

$tempBase = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\')
$verificationRoot = Join-Path $tempBase ("rbcsp-package-verify-" + [Guid]::NewGuid().ToString('N'))
$verificationRoot = [System.IO.Path]::GetFullPath($verificationRoot)
Assert-Condition ($verificationRoot.StartsWith($tempBase + '\', [StringComparison]::OrdinalIgnoreCase)) 'Verification root escaped the system temp directory.'
Assert-Condition ([System.IO.Path]::GetFileName($verificationRoot).StartsWith('rbcsp-package-verify-', [StringComparison]::Ordinal)) 'Unexpected verification root name.'

$unicodePath = 'RBCSP verify ' + [char]0x96ea + '\candidate ' + [char]0x5305
$candidateRoot = Join-Path $verificationRoot $unicodePath
$upgradeRoot = Join-Path $verificationRoot 'upgrade files'
$outsideOne = Join-Path $verificationRoot 'outside one'
$outsideTwo = Join-Path $verificationRoot 'outside two'
$outsideThree = Join-Path $verificationRoot 'outside three'
$logRoot = Join-Path $verificationRoot 'logs'
$run = $null
$attachment = $null
$verificationSucceeded = $false

New-Item -ItemType Directory -Path $candidateRoot, $upgradeRoot, $outsideOne, $outsideTwo, $outsideThree, $logRoot -Force | Out-Null

try {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $entries = @($zip.Entries)
        $entryNames = @($entries | ForEach-Object { $_.FullName } | Sort-Object)
        Assert-Condition ($entryNames.Count -eq 12) "ZIP must contain exactly 12 entries; found $($entryNames.Count)."
        Assert-Condition (($entryNames | Select-Object -Unique).Count -eq $entryNames.Count) 'ZIP contains duplicate entries.'
        Assert-Condition (($entryNames -join "`n") -eq ($expectedFiles -join "`n")) 'ZIP entries do not exactly match the frozen Windows allowlist.'
        foreach ($entry in $entries) {
            Assert-Condition (-not $entry.FullName.EndsWith('/')) "ZIP contains a directory entry: $($entry.FullName)"
            Assert-Condition (-not $entry.FullName.Contains('\')) "ZIP entry uses a backslash: $($entry.FullName)"
            Assert-Condition (-not [System.IO.Path]::IsPathRooted($entry.FullName)) "ZIP entry is rooted: $($entry.FullName)"
            Assert-Condition ($entry.FullName -notmatch '(^|/)\.\.(/|$)') "ZIP entry traverses directories: $($entry.FullName)"
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile(
                $entry,
                (Join-Path $candidateRoot $entry.FullName),
                $false
            )
            [System.IO.Compression.ZipFileExtensions]::ExtractToFile(
                $entry,
                (Join-Path $upgradeRoot $entry.FullName),
                $false
            )
        }
    }
    finally {
        $zip.Dispose()
    }

    $dataPath = Join-Path $candidateRoot 'data'
    Assert-Condition (-not (Test-Path -LiteralPath $dataPath)) 'The extracted package must not contain a data directory.'
    $databaseLikeBefore = @(Get-ChildItem -LiteralPath $candidateRoot -Recurse -File | Where-Object {
        $_.Name -match '(?i)(\.db|\.sqlite|\.sqlite3|-wal|-shm)$'
    })
    Assert-Condition ($databaseLikeBefore.Count -eq 0) 'The extracted package contains a database or SQLite sidecar.'

    $version = [System.IO.File]::ReadAllText((Join-Path $candidateRoot 'VERSION')).TrimEnd("`r", "`n")
    Assert-Condition ($version -eq $releaseInputs.releaseVersion) "VERSION contains $version, not $($releaseInputs.releaseVersion)."

    $sumPath = Join-Path $candidateRoot 'SHA256SUMS'
    $sumText = [System.IO.File]::ReadAllText($sumPath)
    Assert-Condition (-not $sumText.Contains("`r")) 'SHA256SUMS must use LF line endings.'
    $sumLines = @($sumText.TrimEnd("`n").Split("`n"))
    Assert-Condition ($sumLines.Count -eq 11) "SHA256SUMS must contain 11 entries; found $($sumLines.Count)."
    $sumNames = New-Object System.Collections.Generic.List[string]
    foreach ($line in $sumLines) {
        Assert-Condition ($line -cmatch '^(?<hash>[0-9a-f]{64})  (?<path>[^/\\]+)$') "Invalid SHA256SUMS line: $line"
        $sumNames.Add($Matches.path)
        $actualHash = Get-LowerSha256 (Join-Path $candidateRoot $Matches.path)
        Assert-Condition ($actualHash -ceq $Matches.hash) "SHA-256 mismatch for $($Matches.path)."
    }
    $expectedSumNames = @($expectedFiles | Where-Object { $_ -ne 'SHA256SUMS' } | Sort-Object)
    Assert-Condition ((@($sumNames) | Sort-Object) -join "`n" -eq ($expectedSumNames -join "`n")) 'SHA256SUMS coverage is not exact.'

    $manifest = Get-Content -LiteralPath (Join-Path $candidateRoot 'MANIFEST.json') -Raw | ConvertFrom-Json
    $provenance = Get-Content -LiteralPath (Join-Path $candidateRoot 'BUILD-PROVENANCE.json') -Raw | ConvertFrom-Json
    $sbom = Get-Content -LiteralPath (Join-Path $candidateRoot 'SBOM.cdx.json') -Raw | ConvertFrom-Json
    $capabilities = Get-Content -LiteralPath (Join-Path $candidateRoot 'FRONTEND-CAPABILITIES.json') -Raw | ConvertFrom-Json
    $notices = [System.IO.File]::ReadAllText((Join-Path $candidateRoot 'THIRD-PARTY-NOTICES.txt'))
    Assert-Condition ($notices.Trim().Length -gt 0) 'THIRD-PARTY-NOTICES.txt is empty.'
    Assert-Condition ([string]$sbom.bomFormat -eq 'CycloneDX' -and [string]$sbom.specVersion -eq '1.6') 'SBOM must be CycloneDX 1.6.'
    Assert-Condition (@($sbom.components).Count -gt 0 -and @($sbom.dependencies).Count -gt 0) 'SBOM must contain components and dependency relationships.'

    Assert-Condition (
        [int]$manifest.schemaVersion -eq 1 -and
        [string]$manifest.packageId -ceq $PackageId -and
        [string]$manifest.archiveName -ceq $package.archiveName -and
        [string]$manifest.version -ceq $releaseInputs.releaseVersion -and
        [string]$manifest.target -ceq $package.target -and
        [string]$manifest.sourceCommit -ceq $SourceCommit -and
        [long]$manifest.sourceDateEpoch -eq $SourceDateEpoch -and
        [int]$manifest.fileCount -eq 10
    ) 'MANIFEST.json identity is invalid.'

    Assert-Condition (
        [int]$capabilities.schemaVersion -eq 1 -and
        [string]$capabilities.kind -ceq 'TARGET_BUILD_ALLOWLIST' -and
        [string]$capabilities.target -ceq 'local' -and
        [string]$capabilities.readiness -ceq 'UI_INTEGRATION_COMPLETE'
    ) 'FRONTEND-CAPABILITIES.json identity is invalid.'
    foreach ($property in @('allowedCapabilities', 'allowedRoutes', 'allowedI18nCatalogs')) {
        $propertyMatch = @($capabilities.PSObject.Properties | Where-Object { $_.Name -ceq $property })
        Assert-Condition (
            $propertyMatch.Count -eq 1 -and
            $propertyMatch[0].Value -is [System.Array]
        ) "FRONTEND-CAPABILITIES.json $property must be an array."
        $values = [string[]]@($propertyMatch[0].Value | ForEach-Object { [string]$_ })
        Assert-Condition (
            $values.Count -gt 0 -and
            @($values | Where-Object { $_.Length -eq 0 }).Count -eq 0
        ) "FRONTEND-CAPABILITIES.json $property must contain non-empty values."
        $unique = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
        foreach ($value in $values) {
            Assert-Condition ($unique.Add($value)) "FRONTEND-CAPABILITIES.json $property contains a duplicate value: $value"
        }
    }

    $executablePath = Join-Path $candidateRoot 'RBCSP.exe'
    Assert-Condition (
        [int]$provenance.schemaVersion -eq 1 -and
        [string]$provenance.packageId -ceq $PackageId -and
        [string]$provenance.source.commit -ceq $SourceCommit -and
        [long]$provenance.source.dateEpoch -eq $SourceDateEpoch -and
        [string]$provenance.build.target -ceq $package.target -and
        [string]$provenance.artifact.path -ceq 'RBCSP.exe' -and
        [string]$provenance.artifact.sha256 -ceq (Get-LowerSha256 $executablePath)
    ) 'BUILD-PROVENANCE.json identity is invalid.'
    Assert-Condition (
        [int]$sbom.version -eq 1 -and
        [string]$sbom.metadata.component.version -ceq $releaseInputs.releaseVersion
    ) 'SBOM root identity is invalid.'

    $dumpbin = Get-DumpBin
    $imports = @(& $dumpbin /NOLOGO /DEPENDENTS $executablePath)
    Assert-Condition ($LASTEXITCODE -eq 0) "dumpbin /DEPENDENTS failed with exit code $LASTEXITCODE."
    $dlls = @($imports | ForEach-Object {
        if ($_ -match '^\s*(?<dll>[^\s]+\.dll)\s*$') { $Matches.dll }
    })
    $forbiddenImports = @($dlls | Where-Object {
        $_ -match '(?i)^(vcruntime|msvcp|msvcr|concrt).*\.dll$' -or
        $_ -match '(?i)^ucrtbase\.dll$' -or
        $_ -match '(?i)^api-ms-win-crt-.*\.dll$'
    })
    Assert-Condition ($forbiddenImports.Count -eq 0) "RBCSP.exe dynamically imports a C/C++ runtime: $($forbiddenImports -join ', ')"

    $run = Start-Candidate $candidateRoot $outsideOne 'first' $logRoot
    $first = Read-Bootstrap $run.Origin
    Assert-Condition ([int]$first.protocolVersion -eq 1) 'Local bootstrap protocol version is not 1.'
    Assert-Condition ([string]$first.data.mode -ceq 'LOCAL') 'Local bootstrap mode is not LOCAL.'
    $firstNonce = [string]$first.data.sessionNonce
    Assert-Condition ($firstNonce -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$') 'Local bootstrap session nonce is not a UUIDv4.'
    Assert-Condition (@($first.data.state.savedViews).Count -eq 0) 'First-run Saved views are not empty.'
    Assert-Condition (@($first.data.state.selectedSections).Count -eq 0) 'First-run selected Sections are not empty.'
    Assert-Condition (@($first.data.state.desiredWatches).Count -eq 0) 'First-run desired watches are not empty.'
    Assert-Condition (@($first.data.state.episodeHistory.items).Count -eq 0) 'First-run local history is not empty.'
    Assert-Condition ($null -eq $first.data.state.currentFilters.value) 'First-run current filters are not empty.'
    Assert-Condition ([int]$first.data.state.activeWatchCount -eq 0) 'First-run active watch count is not zero.'

    $serviceStatus = Read-ServiceStatus $run.Origin
    Assert-Condition ([int]$serviceStatus.protocolVersion -eq 1) 'Service Status protocol version is not 1.'
    Assert-Condition ([int]$serviceStatus.data.contractVersion -eq 2) 'Service Status contract version is not 2.'
    $currentWatchableTerm = [string]$serviceStatus.data.termWindow.currentTerm
    Assert-Condition ($currentWatchableTerm -match '^[0179][0-9]{4}$') 'Service Status did not expose a valid current Rutgers term.'

    if ($runBrowserSmoke) {
        Invoke-BrowserSmoke $run.Origin $BrowserSmokeScript $PlaywrightRoot $NodePath
        $postBrowser = Read-Bootstrap $run.Origin
        Assert-Condition ([string]$postBrowser.data.sessionNonce -ceq $firstNonce) 'Browser acceptance changed the local session nonce.'
        Assert-EmptyPersonalState $postBrowser 'Post-browser Reset'
        $first = $postBrowser
    }

    # The Section the fixture publishes. Not an arbitrary marker: the restart
    # lifetime below has to ARM it, and a Section the catalog does not publish
    # can only ever prove that the intent survived.
    $marker = [ordered]@{ term = $currentWatchableTerm; campus = 'NB'; index = '10001' }
    $selectionBody = [ordered]@{
        protocolVersion = 1
        payload = [ordered]@{
            expectedUserStateRevision = [long]$first.data.state.stateRevision
            sections = @($marker)
        }
    } | ConvertTo-Json -Depth 8 -Compress
    $headers = @{
        Origin = $run.Origin.TrimEnd('/')
        'x-bcsp-session' = $firstNonce
    }
    $selectionResponse = Invoke-RestMethod -UseBasicParsing -Method Put `
        -Uri ($run.Origin + 'api/v1/local/selection') -Headers $headers `
        -ContentType 'application/json' -Body $selectionBody -TimeoutSec 10
    Assert-Condition ([int]$selectionResponse.protocolVersion -eq 1) 'Selection update did not return protocol version 1.'

    # Standing watch intent, written the way a page writes it. The authority
    # deliberately does not consult the catalog, so this commits in a
    # catalog-less smoke run -- which is the point: the non-empty restore and
    # reset paths are the ones that can be wrong, and an empty-table
    # rehearsal exercises neither.
    $emptyAuthority = Read-DesiredWatch $run.Origin
    Assert-Condition ([int]$emptyAuthority.protocolVersion -eq 1) 'Desired-watch read did not return protocol version 1.'
    Assert-Condition ([int]$emptyAuthority.data.contractVersion -eq 1) 'Desired-watch read did not return contract version 1.'
    Assert-Condition (@($emptyAuthority.data.entries).Count -eq 0) 'First-run desired-watch authority is not empty.'
    $firstGeneration = [long]$emptyAuthority.data.authorityGeneration
    Assert-Condition ($firstGeneration -ge 1) 'Desired-watch authority generation is not positive.'

    $markerPolicy = [ordered]@{
        notificationMode = 'ONE_SHOT'
        maxAudible = 3
        continuousDuration = [ordered]@{ kind = 'FINITE'; seconds = 600 }
    }
    $committedIntent = Write-DesiredWatch $run.Origin $headers $marker $firstGeneration 0 $markerPolicy
    Assert-Condition ([string]$committedIntent.data.outcome -ceq 'COMMITTED') 'Desired-watch write was not committed.'
    Assert-Condition ($committedIntent.data.replayed -eq $false) 'A first desired-watch write reported itself as a replay.'
    Assert-Condition (@($committedIntent.data.state.entries).Count -eq 1) 'The desired-watch write did not return the state it produced.'

    $firstAuthority = Read-DesiredWatch $run.Origin
    Assert-Condition (@($firstAuthority.data.entries).Count -eq 1) 'Desired-watch intent was not stored.'
    Assert-Condition ($null -ne $firstAuthority.data.entries[0].policy) 'Stored desired-watch intent is a tombstone.'
    $firstBootstrap = Read-Bootstrap $run.Origin
    Assert-Condition (@($firstBootstrap.data.state.desiredWatches).Count -eq 1) 'Bootstrap did not expose the stored desired watch.'

    $databasePath = Join-Path $candidateRoot 'data\rbcsp.sqlite'
    Assert-Condition (Test-Path -LiteralPath $databasePath -PathType Leaf) 'First run did not create data/rbcsp.sqlite.'

    Stop-CandidateGracefully $run $firstNonce
    $run = $null
    $databaseHeader = [System.IO.File]::ReadAllBytes($databasePath)[0..15]
    Assert-Condition ([System.Text.Encoding]::ASCII.GetString($databaseHeader) -ceq "SQLite format 3`0") 'The runtime database does not have the SQLite 3 header.'
    $sidecars = @(Get-ChildItem -LiteralPath (Join-Path $candidateRoot 'data') -File | Where-Object { $_.Name -match '(?i)(-wal|-shm)$' })
    Assert-Condition ($sidecars.Count -eq 0) 'SQLite WAL/SHM sidecars remained after graceful exit.'
    $databaseHashBeforeUpgrade = Get-LowerSha256 $databasePath

    foreach ($name in $expectedFiles) {
        Copy-Item -LiteralPath (Join-Path $upgradeRoot $name) -Destination (Join-Path $candidateRoot $name) -Force
    }
    Assert-Condition ((Get-LowerSha256 $databasePath) -ceq $databaseHashBeforeUpgrade) 'Replacing release files changed the package-local database.'

    # Seeded here, with the candidate stopped: after the file-replacement
    # invariant has been measured on an untouched database, and before the
    # lifetime that has to materialize the stored intent.
    Invoke-FixtureSeeder $candidateRoot $currentWatchableTerm

    $run = Start-Candidate $candidateRoot $outsideTwo 'second' $logRoot -UseLauncher
    $second = Read-Bootstrap $run.Origin
    $secondNonce = [string]$second.data.sessionNonce
    Assert-Condition ($secondNonce -ne $firstNonce) 'Restart reused the previous local session nonce.'
    $selected = @($second.data.state.selectedSections)
    Assert-Condition ($selected.Count -eq 1) 'Restart did not restore exactly one selected Section.'
    Assert-Condition (
        [string]$selected[0].term -ceq $marker.term -and
        [string]$selected[0].campus -ceq $marker.campus -and
        [string]$selected[0].index -ceq $marker.index
    ) 'Restart did not restore the package-local synthetic selection marker.'
    $sqliteFiles = @(Get-ChildItem -LiteralPath $candidateRoot -Recurse -File | Where-Object { $_.Name -match '(?i)\.(db|sqlite|sqlite3)$' })
    Assert-Condition ($sqliteFiles.Count -eq 1 -and $sqliteFiles[0].FullName -eq $databasePath) 'The candidate did not use exactly one package-local database.'

    # The intent survived the restart, under the same authority generation --
    # a restart is not a reset, so nothing a page read before it has become
    # stale. Nothing is materialized, because no page has attached, and the
    # read says so plainly rather than implying a watch is running.
    $secondAuthority = Read-DesiredWatch $run.Origin
    Assert-Condition (@($secondAuthority.data.entries).Count -eq 1) 'Restart did not restore the stored desired watch.'
    $restored = $secondAuthority.data.entries[0]
    Assert-Condition (
        [string]$restored.section.term -ceq $marker.term -and
        [string]$restored.section.campus -ceq $marker.campus -and
        [string]$restored.section.index -ceq $marker.index
    ) 'Restart restored a different desired-watch Section.'
    Assert-Condition ($null -ne $restored.policy) 'Restart turned the stored intent into a tombstone.'
    Assert-Condition ($null -eq $restored.materialized) 'Nothing has attached, so nothing may be reported as materialized.'
    Assert-Condition ([long]$secondAuthority.data.authorityGeneration -eq $firstGeneration) 'A restart moved the desired-watch authority generation.'
    Assert-Condition (@((Read-Bootstrap $run.Origin).data.state.desiredWatches).Count -eq 1) 'Restart bootstrap lost the stored desired watch.'
    Assert-Condition (
        [int](Read-Bootstrap $run.Origin).data.state.activeWatchCount -eq 0
    ) 'Nothing has attached, so the candidate must hold no watch.'

    # A page attaches. THIS is the restore the milestone is about: the
    # candidate reads the stored row and puts a real watch behind it, and the
    # read reports the same generation, revision, epoch and policy the
    # authority holds. Anything less is "the row survived", which is a
    # different and much weaker claim.
    $attachment = Open-WatchSocket $run.Origin $secondNonce
    $materialization = Wait-DesiredWatchMaterialized $run.Origin $marker
    $armed = $materialization.Entry
    $armedState = $materialization.State
    Assert-Condition ($null -ne $armed.policy) 'A materialized entry must still be a watch, not a tombstone.'
    Assert-Condition ($null -eq $armed.failure) "The restored watch reported a failure: $($armed | ConvertTo-Json -Depth 8 -Compress)"
    Assert-Condition ($armed.pendingDisarm -eq $false) 'The restored watch reported itself as stopping.'
    Assert-Condition (
        [long]$armed.materialized.authorityGeneration -eq [long]$armedState.data.authorityGeneration -and
        [long]$armed.materialized.revision -eq [long]$armed.revision -and
        [long]$armed.materialized.materializationEpoch -eq [long]$armed.materializationEpoch
    ) 'The materialized stamp does not match the authority it was armed under.'
    Assert-Condition (
        [string]$armed.materialized.policy.notificationMode -ceq [string]$armed.policy.notificationMode -and
        [long]$armed.materialized.policy.maxAudible -eq [long]$armed.policy.maxAudible -and
        [string]$armed.materialized.policy.continuousDuration.kind -ceq [string]$armed.policy.continuousDuration.kind
    ) 'The running watch is not running the policy the authority holds.'
    Assert-Condition (
        [string]$armed.materialized.activeWatchId -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
    ) 'The running watch is not addressable.'
    Assert-Condition (
        [long]$armedState.data.authorityGeneration -eq $firstGeneration
    ) 'Materializing moved the desired-watch authority generation.'
    Assert-Condition (
        [int](Read-Bootstrap $run.Origin).data.state.activeWatchCount -eq 1
    ) 'The candidate did not report the watch it is really holding.'

    $secondHeaders = @{
        Origin = $run.Origin.TrimEnd('/')
        'x-bcsp-session' = $secondNonce
    }
    $prepareResetBody = [ordered]@{
        protocolVersion = 1
        payload = [ordered]@{
            expectedUserStateRevision = [long]$second.data.state.stateRevision
        }
    } | ConvertTo-Json -Depth 6 -Compress
    $preparedReset = Invoke-RestMethod -UseBasicParsing -Method Post `
        -Uri ($run.Origin + 'api/v1/local/user-data-reset/prepare') -Headers $secondHeaders `
        -ContentType 'application/json' -Body $prepareResetBody -TimeoutSec 10
    Assert-Condition ([int]$preparedReset.protocolVersion -eq 1) 'Full Reset preparation did not return protocol version 1.'
    $confirmationToken = [string]$preparedReset.data.confirmationToken
    Assert-Condition ($confirmationToken -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$') 'Full Reset preparation did not return a UUIDv4 confirmation token.'
    Assert-Condition (
        [long]$preparedReset.data.expectedUserStateRevision -eq [long]$second.data.state.stateRevision
    ) 'Full Reset preparation returned the wrong user-state revision.'
    Assert-Condition ([long]$preparedReset.data.expiresInSeconds -gt 0) 'Full Reset confirmation token is already expired.'

    $confirmResetBody = [ordered]@{
        protocolVersion = 1
        payload = [ordered]@{ confirmationToken = $confirmationToken }
    } | ConvertTo-Json -Depth 6 -Compress
    $confirmedReset = Invoke-RestMethod -UseBasicParsing -Method Post `
        -Uri ($run.Origin + 'api/v1/local/user-data-reset/confirm') -Headers $secondHeaders `
        -ContentType 'application/json' -Body $confirmResetBody -TimeoutSec 10
    Assert-Condition ([int]$confirmedReset.protocolVersion -eq 1) 'Full Reset confirmation did not return protocol version 1.'
    Assert-Condition ([long]$confirmedReset.data.deletedSelectedSections -eq 1) 'Full Reset did not delete the synthetic selected Section.'
    # Presence plus value, not value alone: [long]$null is 0, so a field that
    # exists but is null would slip past the count check. (A field that is
    # absent entirely already throws under Set-StrictMode -Version Latest.)
    # These are the counts migration 10004 introduced, and this is the run
    # that can find them wrong: the table is NOT empty, so a deletion that
    # silently did nothing would report one here.
    Assert-Condition (
        ($null -ne $confirmedReset.data.deletedDesiredWatches) -and
        ([long]$confirmedReset.data.deletedDesiredWatches -eq 1)
    ) 'Full Reset did not delete the stored desired watch.'
    Assert-Condition (
        ($null -ne $confirmedReset.data.deletedDesiredWatchReceipts) -and
        ([long]$confirmedReset.data.deletedDesiredWatchReceipts -eq 1)
    ) 'Full Reset did not delete the desired-watch receipt.'
    $resetAuthority = Read-DesiredWatch $run.Origin
    Assert-Condition (@($resetAuthority.data.entries).Count -eq 0) 'Full Reset left desired-watch rows behind.'
    Assert-Condition (
        [long]$resetAuthority.data.authorityGeneration -gt $firstGeneration
    ) 'Full Reset did not raise the desired-watch authority generation.'
    $resetGeneration = [long]$resetAuthority.data.authorityGeneration
    Assert-Condition (
        [long]$confirmedReset.data.stateRevision -gt [long]$second.data.state.stateRevision
    ) 'Full Reset did not advance the user-state revision.'

    $afterReset = Read-Bootstrap $run.Origin
    Assert-EmptyPersonalState $afterReset 'Post-Reset'
    Assert-Condition (
        [int]$afterReset.data.state.activeWatchCount -eq 0
    ) 'Full Reset emptied the authority while the candidate kept holding a watch.'
    Close-WatchSocket $attachment
    $attachment = $null
    Assert-Condition (Test-Path -LiteralPath $databasePath -PathType Leaf) 'Full Reset deleted the package-local database.'
    $sqliteFilesAfterReset = @(Get-ChildItem -LiteralPath $candidateRoot -Recurse -File | Where-Object { $_.Name -match '(?i)\.(db|sqlite|sqlite3)$' })
    Assert-Condition ($sqliteFilesAfterReset.Count -eq 1 -and $sqliteFilesAfterReset[0].FullName -eq $databasePath) 'Full Reset changed the exactly-one package-local database boundary.'
    Stop-CandidateGracefully $run $secondNonce
    $run = $null

    $run = Start-Candidate $candidateRoot $outsideThree 'third' $logRoot -UseLauncher
    $third = Read-Bootstrap $run.Origin
    $thirdNonce = [string]$third.data.sessionNonce
    Assert-Condition ($thirdNonce -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$') 'Third-launch session nonce is not a UUIDv4.'
    Assert-Condition ($thirdNonce -ne $firstNonce -and $thirdNonce -ne $secondNonce) 'Third launch reused an earlier local session nonce.'
    Assert-EmptyPersonalState $third 'Restart-after-Reset'
    $thirdAuthority = Read-DesiredWatch $run.Origin
    Assert-Condition (@($thirdAuthority.data.entries).Count -eq 0) 'Restart after full Reset restored desired-watch rows.'
    Assert-Condition (
        [long]$thirdAuthority.data.authorityGeneration -eq $resetGeneration
    ) 'Restart after full Reset did not keep the raised authority generation.'
    Assert-Condition (Test-Path -LiteralPath $databasePath -PathType Leaf) 'Restart after full Reset lost the package-local database.'
    $finalSqliteFiles = @(Get-ChildItem -LiteralPath $candidateRoot -Recurse -File | Where-Object { $_.Name -match '(?i)\.(db|sqlite|sqlite3)$' })
    Assert-Condition ($finalSqliteFiles.Count -eq 1 -and $finalSqliteFiles[0].FullName -eq $databasePath) 'Restart after full Reset did not retain exactly one package-local database.'
    Stop-CandidateGracefully $run $thirdNonce
    $run = $null

    $archiveHash = Get-LowerSha256 $ArchivePath
    $verificationSucceeded = $true
    [ordered]@{
        status = 'PASS'
        archive = [System.IO.Path]::GetFileName($ArchivePath)
        sha256 = $archiveHash
        files = $expectedFiles.Count
        restarts = 2
        desiredWatchCycle = 'NON_EMPTY_MATERIALIZED'
        materializedSection = "$($marker.term)/$($marker.campus)/$($marker.index)"
    } | ConvertTo-Json -Compress
}
finally {
    Close-WatchSocket $attachment
    Stop-CandidateAfterFailure $run
    if ($verificationSucceeded -and (Test-Path -LiteralPath $verificationRoot)) {
        $resolved = [System.IO.Path]::GetFullPath($verificationRoot)
        if ($resolved.StartsWith($tempBase + '\', [StringComparison]::OrdinalIgnoreCase) -and
            [System.IO.Path]::GetFileName($resolved).StartsWith('rbcsp-package-verify-', [StringComparison]::Ordinal)) {
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
    }
}
