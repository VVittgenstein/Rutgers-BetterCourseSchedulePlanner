[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SourceRoot,

    [string]$SourceCommit,

    [string]$OutputRoot = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'release\0.1.3'),

    [string]$BuildRoot = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) '.cache\product-build\windows')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$PackageId = 'WINDOWS_LOCAL_RELEASE_ARCHIVE'
$Target = 'x86_64-pc-windows-msvc'
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Resolve-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    return [System.IO.Path]::GetFullPath($Path)
}

function Resolve-ShortDirectoryPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $fileSystem = New-Object -ComObject Scripting.FileSystemObject
    try {
        $shortPath = [string]$fileSystem.GetFolder($Path).ShortPath
        if ([string]::IsNullOrWhiteSpace($shortPath)) {
            throw "Windows did not return a short path for $Path."
        }
        if (-not [System.IO.Path]::IsPathRooted($shortPath) -or
            -not (Test-Path -LiteralPath $shortPath -PathType Container)) {
            throw "Windows returned an invalid short path for $Path."
        }
        return $shortPath
    }
    finally {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($fileSystem)
    }
}

function Assert-File {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required file is missing: $Path"
    }
}

function Resolve-ReleaseTool {
    param(
        [Parameter(Mandatory = $true)][string]$PinnedPath,
        [Parameter(Mandatory = $true)][string]$CommandName
    )

    if (Test-Path -LiteralPath $PinnedPath -PathType Leaf) {
        return (Resolve-FullPath $PinnedPath)
    }
    $command = Get-Command $CommandName -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $command) {
        throw "Required release tool was not found: $CommandName"
    }
    return $command.Source
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Label
    )

    Write-Host "==> $Label"
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $FilePath @Arguments 2>&1 | ForEach-Object { Write-Host ([string]$_) }
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "$Label failed with exit code $exitCode."
    }
}

function Invoke-NativeCapture {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][string]$Label
    )

    Write-Host "==> $Label"
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $FilePath @Arguments)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "$Label failed with exit code $exitCode."
    }
    return ($output -join "`n")
}

function Import-MsvcEnvironment {
    $existingCompiler = Get-Command cl.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    $existingLinker = Get-Command link.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not [string]::IsNullOrWhiteSpace($env:VCToolsVersion) -and
        -not [string]::IsNullOrWhiteSpace($env:VCToolsInstallDir) -and
        $existingCompiler -and $existingLinker) {
        Write-Host "==> Reusing the initialized MSVC $($env:VCToolsVersion) environment"
        return
    }

    $candidates = @(
        'C:\Software\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat',
        'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat',
        'C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat',
        'C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Auxiliary\Build\vcvars64.bat',
        'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat'
    )
    $vcvars = $candidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $vcvars) {
        throw 'A Visual Studio 2022 x64 C++ build environment is required.'
    }

    $command = '"' + $vcvars + '" >nul && set'
    $environment = @(& $env:ComSpec /d /s /c $command)
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to initialize the Visual Studio x64 build environment.'
    }
    foreach ($line in $environment) {
        $separator = $line.IndexOf('=')
        if ($separator -gt 0) {
            $name = $line.Substring(0, $separator)
            $value = $line.Substring($separator + 1)
            [Environment]::SetEnvironmentVariable($name, $value, 'Process')
        }
    }
}

function Reset-BuildDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$AllowedParent
    )

    $fullPath = Resolve-FullPath $Path
    $fullParent = (Resolve-FullPath $AllowedParent).TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($fullParent, [StringComparison]::OrdinalIgnoreCase)) {
        throw "BuildRoot must remain under $AllowedParent"
    }
    if (Test-Path -LiteralPath $fullPath) {
        Remove-Item -LiteralPath $fullPath -Recurse -Force
    }
    New-Item -ItemType Directory -Path $fullPath | Out-Null
    return $fullPath
}

function New-DeterministicZip {
    param(
        [Parameter(Mandatory = $true)][string]$PackageRoot,
        [Parameter(Mandatory = $true)][string[]]$Entries,
        [Parameter(Mandatory = $true)][string]$ArchivePath,
        [Parameter(Mandatory = $true)][DateTimeOffset]$Timestamp
    )

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    if (Test-Path -LiteralPath $ArchivePath) {
        Remove-Item -LiteralPath $ArchivePath -Force
    }
    $stream = [System.IO.File]::Open($ArchivePath, [System.IO.FileMode]::CreateNew)
    try {
        $archive = New-Object System.IO.Compression.ZipArchive(
            $stream,
            [System.IO.Compression.ZipArchiveMode]::Create,
            $false
        )
        try {
            foreach ($relativePath in ($Entries | Sort-Object)) {
                $source = Join-Path $PackageRoot $relativePath.Replace('/', '\')
                Assert-File $source
                $entry = $archive.CreateEntry(
                    $relativePath.Replace('\', '/'),
                    [System.IO.Compression.CompressionLevel]::Optimal
                )
                $entry.LastWriteTime = $Timestamp
                $input = [System.IO.File]::OpenRead($source)
                try {
                    $output = $entry.Open()
                    try {
                        $input.CopyTo($output)
                    }
                    finally {
                        $output.Dispose()
                    }
                }
                finally {
                    $input.Dispose()
                }
            }
        }
        finally {
            $archive.Dispose()
        }
    }
    finally {
        $stream.Dispose()
    }
}

$RepositoryRoot = Resolve-FullPath (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
$SourceRoot = Resolve-FullPath $SourceRoot
$OutputRoot = Resolve-FullPath $OutputRoot
$BuildRoot = Resolve-FullPath $BuildRoot
$ReleaseInputsPath = Join-Path $SourceRoot 'packaging\release-inputs.json'
$MetadataGenerator = Join-Path $SourceRoot 'packaging\generate-release-metadata.mjs'
$AboutConfig = Join-Path $SourceRoot 'packaging\about.toml'
$WindowsTemplates = Join-Path $SourceRoot 'packaging\windows\templates'
$LicensePath = Join-Path $SourceRoot 'LICENSE'

Assert-File $ReleaseInputsPath
Assert-File $LicensePath
Assert-File (Join-Path $WindowsTemplates 'Start-RBCSP.bat')
Assert-File (Join-Path $WindowsTemplates 'README.md')
Assert-File (Join-Path $WindowsTemplates 'QUICKSTART.md')

$sourceHead = (& git -C $SourceRoot rev-parse --verify HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceHead -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') {
    throw 'SourceRoot HEAD is not a full hexadecimal commit id.'
}
if (-not [string]::IsNullOrWhiteSpace($SourceCommit)) {
    if ($SourceCommit -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$') {
        throw 'SourceCommit must be a full lowercase hexadecimal commit id.'
    }
    $resolvedCommit = (& git -C $SourceRoot rev-parse --verify "$SourceCommit`^{commit}").Trim()
    if ($LASTEXITCODE -ne 0 -or $resolvedCommit -cne $SourceCommit) {
        throw 'SourceCommit must resolve to its canonical full commit id.'
    }
    if ($sourceHead -cne $SourceCommit) {
        throw "SourceRoot HEAD is $sourceHead, not requested commit $SourceCommit."
    }
}
else {
    $null = & git -C $SourceRoot symbolic-ref --quiet HEAD 2>$null
    if ($LASTEXITCODE -eq 0) {
        throw 'SourceRoot must be a detached checkout unless SourceCommit is supplied.'
    }
}
$sourceCommit = $sourceHead
$sourceStatus = @(& git -C $SourceRoot status --porcelain --untracked-files=all)
if ($LASTEXITCODE -ne 0 -or $sourceStatus.Count -ne 0) {
    throw 'SourceRoot must be a clean checkout before the build starts.'
}

$releaseInputs = Get-Content -LiteralPath $ReleaseInputsPath -Raw | ConvertFrom-Json
$package = @($releaseInputs.packages | Where-Object { $_.id -eq $PackageId })
if ($package.Count -ne 1) {
    throw "release-inputs.json must define exactly one $PackageId package."
}
$package = $package[0]
if ($package.target -ne $Target -or $package.allowlist.Count -ne 12) {
    throw 'The Windows release target or twelve-file allowlist changed unexpectedly.'
}

$PinnedNodeRoot = Join-Path $RepositoryRoot '.cache\release-tools\node-v24.18.0-win-x64'
$Node = Resolve-ReleaseTool (Join-Path $PinnedNodeRoot 'node.exe') 'node.exe'
$Npm = Resolve-ReleaseTool (Join-Path $PinnedNodeRoot 'npm.cmd') 'npm.cmd'
$NodeRoot = Split-Path -Parent $Node
$PinnedCargoHome = Join-Path $RepositoryRoot '.cache\release-tools\cargo'
$Cargo = Resolve-ReleaseTool (Join-Path $PinnedCargoHome 'bin\cargo.exe') 'cargo.exe'
$CargoHome = if (Test-Path -LiteralPath (Join-Path $PinnedCargoHome 'bin\cargo.exe') -PathType Leaf) {
    $PinnedCargoHome
}
elseif (-not [string]::IsNullOrWhiteSpace($env:CARGO_HOME)) {
    Resolve-FullPath $env:CARGO_HOME
}
else {
    Resolve-FullPath (Join-Path $env:USERPROFILE '.cargo')
}
$PinnedRustupHome = Join-Path $RepositoryRoot '.cache\release-tools\rustup'
$RustupHome = if (Test-Path -LiteralPath $PinnedRustupHome -PathType Container) {
    $PinnedRustupHome
}
elseif (-not [string]::IsNullOrWhiteSpace($env:RUSTUP_HOME)) {
    Resolve-FullPath $env:RUSTUP_HOME
}
else {
    Resolve-FullPath (Join-Path $env:USERPROFILE '.rustup')
}
$CargoCycloneDx = Resolve-ReleaseTool `
    (Join-Path $RepositoryRoot '.cache\release-tools\cargo-cyclonedx\bin\cargo-cyclonedx.exe') `
    'cargo-cyclonedx.exe'
$CargoAbout = Resolve-ReleaseTool `
    (Join-Path $RepositoryRoot '.cache\release-tools\cargo-about\bin\cargo-about.exe') `
    'cargo-about.exe'
foreach ($tool in @($Node, $Npm, $Cargo, $CargoCycloneDx, $CargoAbout)) {
    Assert-File $tool
}

$nodeVersion = (Invoke-NativeCapture $Node @('--version') 'Checking Node.js version').TrimStart('v').Trim()
$npmVersion = (Invoke-NativeCapture $Npm @('--version') 'Checking npm version').Trim()
if ($nodeVersion -ne $releaseInputs.lockedToolchain.node -or $npmVersion -ne $releaseInputs.lockedToolchain.npm) {
    throw "Pinned Node/npm mismatch: found $nodeVersion/$npmVersion."
}

Import-MsvcEnvironment
$env:PATH = "$NodeRoot;$(Join-Path $CargoHome 'bin');$env:PATH"
$env:CARGO_HOME = $CargoHome
$env:RUSTUP_HOME = $RustupHome
$env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'
$env:CARGO_NET_OFFLINE = 'true'
$env:SOURCE_DATE_EPOCH = (& git -C $SourceRoot show -s --format=%ct $sourceCommit).Trim()
$env:TZ = 'UTC'

$allowedBuildParent = Join-Path $RepositoryRoot '.cache\product-build'
$BuildRoot = Reset-BuildDirectory $BuildRoot $allowedBuildParent
$PackageRoot = Join-Path $BuildRoot 'package'
$TargetRoot = Join-Path $BuildRoot 'cargo-target'
$MetadataRoot = Join-Path $BuildRoot 'metadata'
New-Item -ItemType Directory -Path $PackageRoot, $TargetRoot, $MetadataRoot -Force | Out-Null

$frontendRoot = Join-Path $SourceRoot 'frontend'
Push-Location $frontendRoot
try {
    Invoke-Native $Npm @('ci', '--no-audit', '--no-fund') 'Installing the frozen frontend dependency graph'
    Invoke-Native $Npm @('run', 'build:local') 'Building the embedded Windows-local web UI'
}
finally {
    Pop-Location
}

$env:BCSP_LOCAL_WEB_DIST = Join-Path $frontendRoot 'dist\local'
$env:CARGO_TARGET_DIR = $TargetRoot
$pathMapFlags = @('/experimental:deterministic', "/pathmap:$CargoHome=.cargo")
$shortCargoHome = Resolve-ShortDirectoryPath $CargoHome
if (-not $shortCargoHome.Equals($CargoHome, [StringComparison]::OrdinalIgnoreCase)) {
    $pathMapFlags += "/pathmap:$shortCargoHome=.cargo"
}
$pathMapFlags = $pathMapFlags -join ' '
$env:CL = if ([string]::IsNullOrWhiteSpace($env:CL)) { $pathMapFlags } else { "$env:CL $pathMapFlags" }
$rustFlags = @(
    '-C', 'target-feature=+crt-static',
    '-C', 'link-arg=/Brepro',
    "--remap-path-prefix=$SourceRoot=.",
    "--remap-path-prefix=$CargoHome=.cargo"
)
$env:CARGO_ENCODED_RUSTFLAGS = $rustFlags -join [char]0x1f
Invoke-Native $Cargo @(
    'build',
    '--manifest-path', (Join-Path $SourceRoot 'Cargo.toml'),
    '--package', 'bcsp-local',
    '--target', $Target,
    '--release',
    '--locked',
    '--offline'
) 'Building the static-CRT Windows executable'

$builtExecutable = Join-Path $TargetRoot "$Target\release\bcsp-local.exe"
Assert-File $builtExecutable
Copy-Item -LiteralPath $builtExecutable -Destination (Join-Path $PackageRoot 'RBCSP.exe')
Copy-Item -LiteralPath (Join-Path $env:BCSP_LOCAL_WEB_DIST 'capability-manifest.json') `
    -Destination (Join-Path $PackageRoot 'FRONTEND-CAPABILITIES.json')
Copy-Item -LiteralPath $LicensePath -Destination (Join-Path $PackageRoot 'LICENSE')
Copy-Item -LiteralPath (Join-Path $WindowsTemplates 'Start-RBCSP.bat') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $WindowsTemplates 'README.md') -Destination $PackageRoot
Copy-Item -LiteralPath (Join-Path $WindowsTemplates 'QUICKSTART.md') -Destination $PackageRoot
[System.IO.File]::WriteAllText(
    (Join-Path $PackageRoot 'VERSION'),
    "$($releaseInputs.releaseVersion)`n",
    $Utf8NoBom
)

$CycloneOutputBase = '.bcsp-rust-sbom'
Push-Location $SourceRoot
try {
    Invoke-Native $CargoCycloneDx @(
        'cyclonedx',
        '--manifest-path', (Join-Path $SourceRoot 'apps\bcsp-local\Cargo.toml'),
        '--format', 'json',
        '--target', $Target,
        '--spec-version', '1.5',
        '--license-strict',
        '--no-build-deps',
        '--override-filename', $CycloneOutputBase
    ) 'Collecting the Rust runtime SBOM input'
}
finally {
    Pop-Location
}
$rawRustSbom = Join-Path $SourceRoot "apps\bcsp-local\$CycloneOutputBase.json"
Assert-File $rawRustSbom
Copy-Item -LiteralPath $rawRustSbom -Destination (Join-Path $MetadataRoot 'rust-sbom.json') -Force
$sourcePrefix = $SourceRoot.TrimEnd('\') + '\'
$temporarySboms = @(Get-ChildItem -LiteralPath $SourceRoot -Recurse -Force -File -Filter "$CycloneOutputBase.json")
foreach ($temporarySbom in $temporarySboms) {
    if (-not $temporarySbom.FullName.StartsWith($sourcePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "cargo-cyclonedx wrote outside SourceRoot: $($temporarySbom.FullName)"
    }
    Remove-Item -LiteralPath $temporarySbom.FullName -Force
}

Assert-File $AboutConfig
$rawAbout = Join-Path $MetadataRoot 'rust-about.json'
Invoke-Native $CargoAbout @(
    'generate',
    '--manifest-path', (Join-Path $SourceRoot 'apps\bcsp-local\Cargo.toml'),
    '--config', $AboutConfig,
    '--target', $Target,
    '--frozen',
    '--fail',
    '--format', 'json',
    '--output-file', $rawAbout
) 'Collecting Rust license notices'
Assert-File $rawAbout

Assert-File $MetadataGenerator
Invoke-Native $Node @(
    $MetadataGenerator,
    '--source-root', $SourceRoot,
    '--stage-root', $PackageRoot,
    '--package-id', $PackageId,
    '--target', $Target,
    '--binary', (Join-Path $PackageRoot 'RBCSP.exe'),
    '--source-commit', $sourceCommit,
    '--source-date-epoch', $env:SOURCE_DATE_EPOCH,
    '--rustc-version', $releaseInputs.lockedToolchain.rust,
    '--cargo-version', $releaseInputs.lockedToolchain.rust,
    '--node-version', $releaseInputs.lockedToolchain.node,
    '--npm-version', $releaseInputs.lockedToolchain.npm,
    '--cargo-cyclonedx-version', '0.5.9',
    '--cargo-about-version', '0.9.1',
    '--raw-cargo-sbom', (Join-Path $MetadataRoot 'rust-sbom.json'),
    '--raw-cargo-about', $rawAbout
) 'Generating package SBOM, notices, provenance, manifest, and hashes'

$actualFiles = @(Get-ChildItem -LiteralPath $PackageRoot -File | ForEach-Object { $_.Name } | Sort-Object)
$expectedFiles = @($package.allowlist | ForEach-Object { [string]$_ } | Sort-Object)
if (($actualFiles -join "`n") -ne ($expectedFiles -join "`n")) {
    throw "Windows package root does not match the twelve-file allowlist.`nActual: $($actualFiles -join ', ')"
}

New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
$archivePath = Join-Path $OutputRoot $package.archiveName
$timestamp = [DateTimeOffset]::FromUnixTimeSeconds([long]$env:SOURCE_DATE_EPOCH).ToUniversalTime()
New-DeterministicZip $PackageRoot $expectedFiles $archivePath $timestamp
$archiveHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "Created $archivePath"
Write-Host "SHA256 $archiveHash"

[pscustomobject]@{
    archive = $archivePath
    sha256 = $archiveHash
    sourceCommit = $sourceCommit
    sourceDateEpoch = [long]$env:SOURCE_DATE_EPOCH
    fileCount = $expectedFiles.Count
}
