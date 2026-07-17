[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,

    [Parameter(Mandatory = $true)]
    [string]$SourceManifestPath,

    [Parameter(Mandatory = $true)]
    [string]$ProtectedManifestPath
)

$ErrorActionPreference = 'Stop'

$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path

function Get-RelativePath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $absolute = if ([System.IO.Path]::IsPathRooted($Path)) {
        [System.IO.Path]::GetFullPath($Path)
    } else {
        [System.IO.Path]::GetFullPath((Join-Path $root $Path))
    }
    $separator = [System.IO.Path]::DirectorySeparatorChar
    $rootPrefix = $root.TrimEnd($separator) + $separator
    if (-not $absolute.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Path is outside the repository root: $Path"
    }
    $absolute.Substring($rootPrefix.Length).Replace($separator, '/')
}

function Get-FileEvidence {
    param([Parameter(Mandatory = $true)][System.IO.FileInfo]$File)

    [ordered]@{
        path   = Get-RelativePath -Path $File.FullName
        bytes  = $File.Length
        sha256 = (Get-FileHash -LiteralPath $File.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

$stagedPaths = @(
    git -C $root diff --cached --name-only --diff-filter=ACMR
) | Where-Object { $_ -and $_ -notin @(
    (Get-RelativePath -Path $SourceManifestPath),
    (Get-RelativePath -Path $ProtectedManifestPath)
) } | Sort-Object -Unique

if ($LASTEXITCODE -ne 0) {
    throw 'Unable to enumerate the staged recovery baseline.'
}

$sourceEntries = foreach ($relative in $stagedPaths) {
    $absolute = Join-Path $root $relative
    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
        throw "Staged baseline path is not a file: $relative"
    }
    Get-FileEvidence -File (Get-Item -LiteralPath $absolute)
}

$protectedFiles = [System.Collections.Generic.List[System.IO.FileInfo]]::new()

foreach ($pattern in @('chat-log-codex-*.md')) {
    foreach ($file in Get-ChildItem -LiteralPath $root -Filter $pattern -File) {
        $protectedFiles.Add($file)
    }
}

foreach ($relativeRoot in @('HumanTest')) {
    $absoluteRoot = Join-Path $root $relativeRoot
    if (Test-Path -LiteralPath $absoluteRoot) {
        foreach ($file in Get-ChildItem -LiteralPath $absoluteRoot -File -Recurse -Force) {
            $protectedFiles.Add($file)
        }
    }
}

foreach ($relative in @('bcsp-20260122.zip')) {
    $absolute = Join-Path $root $relative
    if (Test-Path -LiteralPath $absolute -PathType Leaf) {
        $protectedFiles.Add((Get-Item -LiteralPath $absolute))
    }
}

$sourceManifest = [ordered]@{
    schemaVersion = 1
    purpose       = 'RC I Round 4 recovered local source baseline'
    generatedAt   = [DateTimeOffset]::UtcNow.ToString('o')
    gitState      = [ordered]@{
        branch  = (git -C $root branch --show-current).Trim()
        commits = [int](git -C $root rev-list --all --count)
        remotes = @((git -C $root remote) | Where-Object { $_ })
    }
    selfExcluded  = @(
        Get-RelativePath -Path $SourceManifestPath
        Get-RelativePath -Path $ProtectedManifestPath
    )
    fileCount     = $sourceEntries.Count
    files         = @($sourceEntries)
}

$protectedEntries = @(
    $protectedFiles |
        Sort-Object FullName -Unique |
        ForEach-Object { Get-FileEvidence -File $_ }
)

$protectedManifest = [ordered]@{
    schemaVersion = 1
    purpose       = 'Assets protected from RC I Round 4 writes and Git staging'
    generatedAt   = [DateTimeOffset]::UtcNow.ToString('o')
    fileCount     = $protectedEntries.Count
    files         = $protectedEntries
}

$sourceDirectory = Split-Path -Parent $SourceManifestPath
$protectedDirectory = Split-Path -Parent $ProtectedManifestPath
[System.IO.Directory]::CreateDirectory($sourceDirectory) | Out-Null
[System.IO.Directory]::CreateDirectory($protectedDirectory) | Out-Null

$sourceJson = $sourceManifest | ConvertTo-Json -Depth 8
$protectedJson = $protectedManifest | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($SourceManifestPath, $sourceJson + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
[System.IO.File]::WriteAllText($ProtectedManifestPath, $protectedJson + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))

Write-Host "Source baseline: $($sourceEntries.Count) files"
Write-Host "Protected assets: $($protectedEntries.Count) files"
