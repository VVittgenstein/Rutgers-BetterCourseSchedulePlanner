[CmdletBinding()]
param(
    [switch]$VerifyOnly,
    [switch]$RequireRemoteCleanReplay
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '04r1a-p7-1-004-r1-windows-golden-line-ending-repair.json'
$baselinePath = Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$evidenceRoot = Join-Path $p7 'evidence\p7-1-004-r1'
$recordsRoot = Join-Path $p7 'records'
$primaryCommit = '008de8c53da39af5562cd8a4022f839100a8a11d'
$primaryParent = '870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38'
$expectedBranch = 'codex/p7-implementation'
$expectedRemote = 'https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
$cargoHome = Join-Path $repo '.cache\p7-1-002\cargo'
$rustupHome = Join-Path $repo '.cache\p7-1-002\rustup'
$toolchainBin = Join-Path $rustupHome 'toolchains\1.97.0-x86_64-pc-windows-msvc\bin'
$cargoExe = Join-Path $cargoHome 'bin\cargo.exe'
$cargoDenyExe = Join-Path $repo '.cache\p7-1-002\tools\cargo-deny\bin\cargo-deny.exe'
$nodeExe = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\runtime\node-v24.18.0-win-x64\node.exe'
$targetDir = Join-Path $repo '.cache\p7-1-004-r1\target'
$scratchRoot = Join-Path $repo '.cache\p7-1-004-r1'
$scratch = $null
$expectedAllowlist = @(
    'crates/bcsp-contracts/tests/wire_golden.rs'
    'project-governance/current/p7/04r1-p7-1-004-r1-windows-golden-line-ending-repair.md'
    'project-governance/current/p7/04r1a-p7-1-004-r1-windows-golden-line-ending-repair.json'
    'project-governance/current/p7/evidence/p7-1-004-r1/commit-allowlist.txt'
    'project-governance/current/p7/evidence/p7-1-004-r1/repair-quality.json'
    'project-governance/current/p7/records/p7-1-004-r1-completion.json'
    'project-governance/current/p7/records/p7-1-004-r1-completion.md'
    'project-governance/current/p7/tools/build-p7-1-004-r1-evidence.ps1'
    'project-governance/current/p7/tools/validate-p7-1-004-r1.ps1'
)
$expectedGovernancePaths = @($expectedAllowlist | Where-Object { $_ -ne 'crates/bcsp-contracts/tests/wire_golden.rs' })

function Normalize-Path([string]$Value) {
    return $Value.Replace('\', '/')
}

function Read-Json([string]$Path) {
    return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json
}

function Get-StringSha256([string]$Value) {
    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Value)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '')
    }
    finally {
        $sha.Dispose()
        [Array]::Clear($bytes, 0, $bytes.Length)
    }
}

function Get-CanonicalText([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        throw "BOM is forbidden in governed text: $Path"
    }
    if ($bytes -contains 0) {
        throw "NUL is forbidden in governed text: $Path"
    }
    try {
        $text = [System.Text.UTF8Encoding]::new($false, $true).GetString($bytes)
    }
    catch {
        throw "governed text is not strict UTF-8: $Path"
    }
    $canonical = $text.Replace("`r`n", "`n")
    if ($canonical.Contains("`r")) {
        throw "lone carriage return is forbidden in governed text: $Path"
    }
    if (-not $canonical.EndsWith("`n", [StringComparison]::Ordinal)) {
        throw "final newline is required in governed text: $Path"
    }
    return $canonical
}

function Get-CanonicalTextSha256([string]$Path) {
    return Get-StringSha256 -Value (Get-CanonicalText -Path $Path)
}

function Write-Utf8([string]$Path, [string]$Content) {
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $parent -Force)
    }
    [System.IO.File]::WriteAllText($Path, $Content.Replace("`r`n", "`n"), [System.Text.UTF8Encoding]::new($false))
}

function Write-Json([string]$Path, [object]$Value) {
    Write-Utf8 -Path $Path -Content (($Value | ConvertTo-Json -Depth 40) + "`n")
}

function Remove-SafeScratch([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $full = [System.IO.Path]::GetFullPath($Path)
    $rootFull = [System.IO.Path]::GetFullPath($scratchRoot).TrimEnd('\')
    if (-not $full.StartsWith("$rootFull\", [StringComparison]::OrdinalIgnoreCase)) {
        throw "scratch cleanup escaped its fixed root: $full"
    }
    $item = Get-Item -LiteralPath $full -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "scratch cleanup target is a reparse point: $full"
    }
    Remove-Item -LiteralPath $full -Recurse -Force
}

function Invoke-Git([string[]]$Arguments, [bool]$AllowFailure = $false, [string]$WorkingDirectory = $repo) {
    $output = @(& git -C $WorkingDirectory @Arguments 2>&1 | ForEach-Object { $_.ToString() })
    $exitCode = $LASTEXITCODE
    if (-not $AllowFailure -and $exitCode -ne 0) {
        throw "git $($Arguments -join ' ') failed: $($output -join ' | ')"
    }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $output }
}

function Get-GitSingle([string[]]$Arguments, [string]$WorkingDirectory = $repo) {
    $result = Invoke-Git -Arguments $Arguments -WorkingDirectory $WorkingDirectory
    $values = @($result.Output | Where-Object { $_ -ne '' })
    if ($values.Count -ne 1) {
        throw "git command did not return exactly one row: $($Arguments -join ' ')"
    }
    return [string]$values[0]
}

function Invoke-CommandCapture([string]$Executable, [string[]]$Arguments, [string]$WorkingDirectory = $repo) {
    if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
        throw "required executable is absent: $Executable"
    }
    Push-Location $WorkingDirectory
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $Executable @Arguments 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }
    if ($exitCode -ne 0) {
        $tail = @($output | Select-Object -Last 12) -join ' | '
        throw "$([System.IO.Path]::GetFileName($Executable)) $($Arguments -join ' ') failed with exit $exitCode`: $tail"
    }
    return @($output)
}

function Compare-Set([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    $actualSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $expectedSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($value in @($Actual)) {
        if (-not $actualSet.Add((Normalize-Path ([string]$value)))) {
            throw "$Label actual set contains a duplicate"
        }
    }
    foreach ($value in @($Expected)) {
        if (-not $expectedSet.Add((Normalize-Path ([string]$value)))) {
            throw "$Label expected set contains a duplicate"
        }
    }
    if (-not $actualSet.SetEquals($expectedSet)) {
        $missing = @($expectedSet | Where-Object { -not $actualSet.Contains($_) } | Sort-Object)
        $extra = @($actualSet | Where-Object { -not $expectedSet.Contains($_) } | Sort-Object)
        throw "$Label set mismatch; missing=$($missing -join ','); extra=$($extra -join ',')"
    }
}

function Test-DeniedTaskPath([string]$Path) {
    $normalized = Normalize-Path $Path
    return $normalized -match '(?i)(^|/)\.secrets(/|$)' -or
        $normalized -match '(?i)(^|/)chat-log-[^/]*\.md$' -or
        $normalized -match '(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)' -or
        $normalized -match '(?i)^project-governance/current/p1/' -or
        $normalized -match '(?i)(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)' -or
        $normalized -match '(?i)(\.sqlite3?|\.db|-wal|-shm)$'
}

function Test-NeverReadProtectedPath([string]$Path) {
    $normalized = Normalize-Path $Path
    return $normalized -match '(?i)(^|/)chat-log-[^/]*\.md$' -or
        $normalized -match '(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)' -or
        $normalized -match '(?i)^project-governance/current/p1/' -or
        $normalized -match '(?i)(^|/)\.secrets(/|$)'
}

function Get-ManifestEntrySetSha256 {
    $lines = @(Get-Content -LiteralPath $baselinePath -Encoding utf8)
    $header = "path`tgit_status`tbaseline_kind`thead_blob_oid`tworktree_sha256`tsize_bytes`tcontent_policy`townership`trequired_action"
    if ($lines.Count -ne 168 -or [string]$lines[0] -cne $header) {
        throw 'protected baseline manifest shape mismatch'
    }
    return Get-StringSha256 -Value ((($lines | Select-Object -Skip 1) -join "`n") + "`n")
}

function Test-ProtectedWorktreeProfile([string[]]$Allowlist) {
    $manifestSha = Get-ManifestEntrySetSha256
    if ($manifestSha -cne 'C7A4FEB33F4F9198678AFA5C38D8CBD4D54378BD18DFA9D22CDA85FA1290089D') {
        throw 'protected baseline manifest entry-set identity mismatch'
    }
    $baseline = @(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")
    if ($baseline.Count -ne 167) {
        throw "protected baseline row count is $($baseline.Count), expected 167"
    }
    $allow = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in @($Allowlist)) { [void]$allow.Add((Normalize-Path $path)) }
    $actual = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    $status = (Invoke-Git -Arguments @('-c', 'core.quotepath=false', 'status', '--porcelain=v1', '--untracked-files=all', '--no-renames')).Output
    foreach ($raw in @($status)) {
        $line = [string]$raw
        if ($line.Length -lt 4) { continue }
        $code = $line.Substring(0, 2)
        $path = Normalize-Path $line.Substring(3)
        if ($allow.Contains($path)) { continue }
        if ($code[0] -ne ' ' -and $code -cne '??') {
            throw 'a protected or foreign path is staged outside the task allowlist'
        }
        $normalizedStatus = if ($code -ceq '??') { 'UNTRACKED' } elseif ($code[1] -ceq 'D') { 'WORKTREE_D' } else { "WORKTREE_$($code[1])" }
        if ($actual.ContainsKey($path)) { throw 'duplicate protected worktree status path' }
        $actual.Add($path, $normalizedStatus)
    }
    $privateIgnored = (Invoke-Git -Arguments @('check-ignore', '-q', '.secrets/') -AllowFailure $true).ExitCode -eq 0
    $privateTracked = @((Invoke-Git -Arguments @('ls-files', '--', '.secrets')).Output | Where-Object { $_ -ne '' })
    if (-not $privateIgnored -or $privateTracked.Count -ne 0) {
        throw 'opaque private root ignore/tracking policy mismatch'
    }
    if ($actual.Count -eq 0) {
        return [pscustomobject]@{ Profile = 'CLEAN_CHECKOUT_0'; Rows = 0; ManifestRows = 167; ManifestEntrySetSha256 = $manifestSha }
    }
    if ($actual.Count -ne 167) {
        throw "partial protected worktree profile is forbidden: observed $($actual.Count) rows"
    }
    Compare-Set -Actual @($actual.Keys) -Expected @($baseline | ForEach-Object { Normalize-Path ([string]$_.path) }) -Label 'protected path set'
    $rowIndex = 0
    foreach ($row in $baseline) {
        $rowIndex++
        $path = Normalize-Path ([string]$row.path)
        $opaqueByPath = Test-NeverReadProtectedPath -Path $path
        $opaqueByPolicy = [string]$row.content_policy -ceq 'OPAQUE_PROTECTED_NO_READ'
        if ($opaqueByPath -and -not $opaqueByPolicy) { throw "protected opaque row $rowIndex has an unsafe content policy" }
        if ([string]$actual[$path] -cne [string]$row.git_status) { throw "protected baseline status mismatch at row $rowIndex" }
        if (-not [string]::IsNullOrWhiteSpace([string]$row.head_blob_oid)) {
            $headBlob = Invoke-Git -Arguments @('rev-parse', "HEAD:$path") -AllowFailure $true
            $headBlobValue = if ($headBlob.ExitCode -eq 0) { (@($headBlob.Output) -join '').Trim() } else { '' }
            if ($headBlobValue -cne [string]$row.head_blob_oid) { throw "protected HEAD blob mismatch at row $rowIndex" }
        }
        $absolute = Join-Path $repo $path.Replace('/', '\')
        if ([string]$row.git_status -ceq 'WORKTREE_D') {
            if (Test-Path -LiteralPath $absolute) { throw "protected deletion reappeared at row $rowIndex" }
            continue
        }
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) { throw "protected file missing at row $rowIndex" }
        if ((Get-Item -LiteralPath $absolute).Length -ne [int64]$row.size_bytes) { throw "protected size mismatch at row $rowIndex" }
        if ($opaqueByPolicy) {
            if ([string]$row.worktree_sha256 -cne 'NOT_COMPUTED') { throw "opaque protected row $rowIndex unexpectedly carries a content hash" }
            continue
        }
        if ($opaqueByPath) { throw "protected never-read row $rowIndex reached the hash branch" }
        if ((Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash -cne [string]$row.worktree_sha256) { throw "protected raw-byte hash mismatch at row $rowIndex" }
    }
    return [pscustomobject]@{ Profile = 'EXACT_PRESERVED_167'; Rows = 167; ManifestRows = 167; ManifestEntrySetSha256 = $manifestSha }
}

function Get-ProtectedPathSet {
    $set = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($row in @(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")) { [void]$set.Add((Normalize-Path ([string]$row.path))) }
    return $set
}

function Get-TaskStatusPaths {
    $protected = Get-ProtectedPathSet
    $paths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($raw in @((Invoke-Git -Arguments @('-c', 'core.quotepath=false', 'status', '--porcelain=v1', '--untracked-files=all', '--no-renames')).Output)) {
        $line = [string]$raw
        if ($line.Length -lt 4) { continue }
        $path = Normalize-Path $line.Substring(3)
        if ($protected.Contains($path)) { continue }
        if (Test-DeniedTaskPath $path) { throw "denied path entered task boundary: $path" }
        [void]$paths.Add($path)
    }
    return @($paths | Sort-Object)
}

function Get-CommitPaths([string]$Revision) {
    return @((Invoke-Git -Arguments @('diff-tree', '--no-renames', '--no-commit-id', '--name-only', '-r', $Revision)).Output | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path $_ })
}

function Test-PublicationText([string]$Content, [string]$Label) {
    $patterns = @(
        '-----BEGIN [A-Z ]*PRIVATE KEY-----',
        '(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}',
        'github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}',
        'npm_[A-Za-z0-9]{20,}',
        'sk-(proj-)?[A-Za-z0-9_-]{20,}',
        'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}',
        '(?i)https?://[^/\s:@]+:[^/\s@]+@',
        '(?i)(^|[\s"''])[A-Z]:[\\/]',
        '(?i)/(home|Users|mnt/[a-z])/[^/\s"'']+/',
        '(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}'
    )
    foreach ($pattern in $patterns) {
        if ($Content -match $pattern) { throw "publication safety pattern matched in $Label" }
    }
}

function Test-PublicationFiles([string[]]$Paths) {
    foreach ($path in @($Paths)) {
        if (Test-DeniedTaskPath $path) { throw "denied publication path: $path" }
        $absolute = Join-Path $repo $path.Replace('/', '\')
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) { throw "publication file missing: $path" }
        Test-PublicationText -Content (Get-CanonicalText -Path $absolute) -Label $path
    }
}

function Test-SourceContract {
    $path = Join-Path $repo 'crates\bcsp-contracts\tests\wire_golden.rs'
    $text = Get-Content -LiteralPath $path -Raw -Encoding utf8
    $required = @(
        'fn canonical_golden(expected: &str) -> String',
        'expected.replace("\r\n", "\n")',
        "!canonical.contains('\r')",
        'assert_eq!(pretty(value), canonical_golden(expected));',
        'fn golden_comparison_is_portable_across_windows_checkouts()',
        'canonical_golden("{\r\n  \"version\": 1\r\n}\r\n")'
    )
    foreach ($needle in $required) {
        if ($text.IndexOf($needle, [StringComparison]::Ordinal) -lt 0) { throw "repair source contract missing: $needle" }
    }
    if ([regex]::Matches($text, [regex]::Escape('assert_golden(&value, golden);')).Count -ne 9) {
        throw 'all nine golden comparisons must remain routed through assert_golden'
    }
}

function Test-PrimaryBoundary {
    $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', $primaryCommit)) -split '\s+'
    if ($parents.Count -ne 2 -or $parents[1] -cne $primaryParent) { throw 'immutable primary topology mismatch' }
    $primaryContract = Read-Json -Path (Join-Path $p7 '04a-p7-1-004-shared-domain-identity-and-typed-api-schema.json')
    $primaryCompletion = Read-Json -Path (Join-Path $recordsRoot 'p7-1-004-completion.json')
    $primaryAllowlist = @(Get-Content -LiteralPath (Join-Path $p7 'evidence\p7-1-004\commit-allowlist.txt') -Encoding utf8 | Where-Object { $_ -ne '' })
    $contractAllowlist = @($primaryContract.commitBoundary.taskCommitAllowlist | ForEach-Object { [string]$_ })
    $completionAllowlist = @($primaryCompletion.commitAllowlist | ForEach-Object { [string]$_ })
    if ($contractAllowlist.Count -ne 41 -or $primaryAllowlist.Count -ne 41 -or $completionAllowlist.Count -ne 41) { throw 'immutable primary allowlist cardinality mismatch' }
    Compare-Set -Actual @(Get-CommitPaths -Revision $primaryCommit) -Expected $contractAllowlist -Label 'primary commit/contract'
    Compare-Set -Actual $primaryAllowlist -Expected $contractAllowlist -Label 'primary evidence/contract'
    Compare-Set -Actual $completionAllowlist -Expected $contractAllowlist -Label 'primary completion/contract'
}

function Test-RepairBoundary([string[]]$Allowlist, [string[]]$RequiredGovernancePaths) {
    $head = Get-GitSingle -Arguments @('rev-parse', 'HEAD')
    if ($head -ceq $primaryCommit) {
        $candidate = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($path in @(Get-TaskStatusPaths)) { [void]$candidate.Add($path) }
        foreach ($path in @($RequiredGovernancePaths)) { [void]$candidate.Add((Normalize-Path $path)) }
        Compare-Set -Actual @($candidate) -Expected $Allowlist -Label 'prospective repair boundary'
        return
    }
    $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', 'HEAD')) -split '\s+'
    if ($parents.Count -ne 2 -or $parents[1] -cne $primaryCommit) { throw 'repair is not the direct single-parent child of P7.1-004' }
    Compare-Set -Actual @(Get-CommitPaths -Revision 'HEAD') -Expected $Allowlist -Label 'committed repair boundary'
}

function Invoke-AutocrlfSelfTest {
    $root = Join-Path $scratchRoot ("autocrlf-selftest-" + [Guid]::NewGuid().ToString('N'))
    $source = Join-Path $root 'source'
    $clone = Join-Path $root 'clone'
    [void](New-Item -ItemType Directory -Path $source -Force)
    try {
        [void](Invoke-Git -Arguments @('init', '--quiet') -WorkingDirectory $source)
        [void](Invoke-Git -Arguments @('config', 'user.name', 'P7 Validation') -WorkingDirectory $source)
        [void](Invoke-Git -Arguments @('config', 'user.email', 'p7-validation.invalid') -WorkingDirectory $source)
        [void](Invoke-Git -Arguments @('config', 'core.autocrlf', 'false') -WorkingDirectory $source)
        $sourceFixture = Join-Path $repo 'crates\bcsp-contracts\tests\golden\section-key-v1.json'
        $isolatedFixture = Join-Path $source 'fixture.json'
        Write-Utf8 -Path $isolatedFixture -Content (Get-CanonicalText -Path $sourceFixture)
        [void](Invoke-Git -Arguments @('add', '--', 'fixture.json') -WorkingDirectory $source)
        [void](Invoke-Git -Arguments @('commit', '--quiet', '-m', 'fixture') -WorkingDirectory $source)
        $cloneResult = @(& git -c core.autocrlf=true clone --quiet --no-local $source $clone 2>&1 | ForEach-Object { $_.ToString() })
        if ($LASTEXITCODE -ne 0) { throw "isolated autocrlf clone failed: $($cloneResult -join ' | ')" }
        $status = @((Invoke-Git -Arguments @('status', '--porcelain=v1', '--untracked-files=all') -WorkingDirectory $clone).Output | Where-Object { $_ -ne '' })
        if ($status.Count -ne 0) { throw 'isolated autocrlf clone is not normally clean' }
        $eol = Get-GitSingle -Arguments @('ls-files', '--eol', '--', 'fixture.json') -WorkingDirectory $clone
        if ($eol -notmatch 'i/lf\s+w/crlf') { throw "isolated checkout did not expose LF-to-CRLF conversion: $eol" }
        $blob = Get-GitSingle -Arguments @('rev-parse', 'HEAD:fixture.json') -WorkingDirectory $clone
        $rawWorktree = Get-GitSingle -Arguments @('hash-object', '--no-filters', '--', 'fixture.json') -WorkingDirectory $clone
        if ($blob -ceq $rawWorktree) { throw 'isolated CRLF worktree did not diverge from LF blob bytes' }
        if ((Get-CanonicalTextSha256 -Path (Join-Path $source 'fixture.json')) -cne (Get-CanonicalTextSha256 -Path (Join-Path $clone 'fixture.json'))) {
            throw 'isolated LF and CRLF canonical hashes differ'
        }
        return [pscustomobject]@{ State = 'PASS'; ObservedCrlfFiles = 1; RawBlobMismatches = 1; NormalStatusRows = 0 }
    }
    finally {
        Remove-SafeScratch -Path $root
    }
}

$contract = Read-Json -Path $contractPath
if ([int]$contract.schemaVersion -ne 1 -or [string]$contract.taskId -cne 'P7.1-004-R1' -or [string]$contract.state -cne 'REPAIR_CONTRACT') { throw 'repair contract identity mismatch' }
if ([string]$contract.expectedParent -cne $primaryCommit -or [string]$contract.branch -cne $expectedBranch) { throw 'repair contract Git boundary mismatch' }
$allowlist = @($contract.commitBoundary.taskCommitAllowlist | ForEach-Object { Normalize-Path ([string]$_) })
if ($allowlist.Count -ne 9 -or [int]$contract.commitBoundary.expectedTaskPathCount -ne 9) { throw 'repair allowlist cardinality mismatch' }
Compare-Set -Actual $allowlist -Expected $expectedAllowlist -Label 'contract/hard-coded repair allowlist'
Compare-Set -Actual @($contract.commitBoundary.requiredGovernancePaths | ForEach-Object { [string]$_ }) -Expected $expectedGovernancePaths -Label 'contract/hard-coded governance paths'
$sorted = [string[]]$allowlist.Clone()
[Array]::Sort($sorted, [StringComparer]::Ordinal)
for ($index = 0; $index -lt $allowlist.Count; $index++) { if ($allowlist[$index] -cne $sorted[$index]) { throw 'repair allowlist is not ordinal sorted' } }
foreach ($path in $allowlist) { if (Test-DeniedTaskPath $path) { throw "denied path in repair allowlist: $path" } }
Compare-Set -Actual @(Get-Content -LiteralPath (Join-Path $evidenceRoot 'commit-allowlist.txt') -Encoding utf8 | Where-Object { $_ -ne '' }) -Expected $allowlist -Label 'checked-in repair allowlist'

if ((Get-GitSingle -Arguments @('symbolic-ref', '--short', 'HEAD')) -cne $expectedBranch) { throw 'unexpected branch' }
if ((Get-GitSingle -Arguments @('remote', 'get-url', 'origin')) -cne $expectedRemote) { throw 'unexpected origin URL' }
Test-PrimaryBoundary
Test-SourceContract

$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$env:CARGO_TARGET_DIR = $targetDir
$env:PATH = "$toolchainBin;$($cargoHome)\bin;$env:PATH"
[void](New-Item -ItemType Directory -Path $targetDir -Force)

$gateIds = [System.Collections.Generic.List[string]]::new()
function Run-Gate([string]$Id, [string]$Executable, [string[]]$Arguments) {
    [void](Invoke-CommandCapture -Executable $Executable -Arguments $Arguments)
    $gateIds.Add($Id)
}

Run-Gate -Id 'rust-graph-guard' -Executable $nodeExe -Arguments @('tools/architecture/verify-rust-graph.mjs', '--json')
Run-Gate -Id 'rust-graph-guard-self-test' -Executable $nodeExe -Arguments @('--test', 'tools/architecture/verify-rust-graph.test.mjs')
Run-Gate -Id 'cargo-metadata-windows-locked-offline' -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc')
Run-Gate -Id 'cargo-metadata-linux-locked-offline' -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-gnu')
$metadataText = @(Invoke-CommandCapture -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1')) -join "`n"
$metadata = $metadataText | ConvertFrom-Json
$gateIds.Add('cargo-metadata-complete-closure-locked-offline')
Run-Gate -Id 'cargo-check-workspace-all-targets-locked-offline' -Executable $cargoExe -Arguments @('check', '--workspace', '--all-targets', '--locked', '--offline')
Run-Gate -Id 'cargo-test-workspace-all-targets-locked-offline' -Executable $cargoExe -Arguments @('test', '--workspace', '--all-targets', '--locked', '--offline')
Run-Gate -Id 'cargo-clippy-workspace-all-targets-locked-offline' -Executable $cargoExe -Arguments @('clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings')
Run-Gate -Id 'cargo-fmt-check' -Executable $cargoExe -Arguments @('fmt', '--all', '--', '--check')
Run-Gate -Id 'cargo-deny-policy-locked-offline' -Executable $cargoDenyExe -Arguments @('--all-features', '--locked', '--offline', 'check', 'advisories', 'bans', 'licenses', 'sources')
Run-Gate -Id 'cargo-test-wire-golden-locked-offline' -Executable $cargoExe -Arguments @('test', '--locked', '--offline', '-p', 'bcsp-contracts', '--test', 'wire_golden')
Run-Gate -Id 'cargo-test-bcsp-contracts-locked-offline' -Executable $cargoExe -Arguments @('test', '--locked', '--offline', '-p', 'bcsp-contracts')
Run-Gate -Id 'cargo-test-bcsp-domain-locked-offline' -Executable $cargoExe -Arguments @('test', '--locked', '--offline', '-p', 'bcsp-domain')

$dependencyPaths = @('Cargo.toml', 'Cargo.lock', 'frontend/package.json', 'frontend/package-lock.json', ':(glob)crates/**/Cargo.toml')
$dependencyDiff = (Invoke-Git -Arguments (@('diff', '--name-only', $primaryCommit, '--') + $dependencyPaths)).Output | Where-Object { $_ -ne '' }
if (@($dependencyDiff).Count -ne 0) { throw 'dependency manifest or lock changed in repair' }
if ((Get-GitSingle -Arguments @('rev-parse', "$primaryCommit`:Cargo.lock")) -cne (Get-GitSingle -Arguments @('hash-object', '--path=Cargo.lock', 'Cargo.lock'))) { throw 'Cargo.lock blob changed' }
if ((Get-GitSingle -Arguments @('rev-parse', "$primaryCommit`:frontend/package-lock.json")) -cne (Get-GitSingle -Arguments @('hash-object', '--path=frontend/package-lock.json', 'frontend/package-lock.json'))) { throw 'npm lock blob changed' }
$thirdParty = @($metadata.packages | Where-Object { $null -ne $_.source } | ForEach-Object { "$($_.name)@$($_.version)" } | Sort-Object -Unique)
if ($thirdParty.Count -eq 0) { throw 'unexpected empty third-party closure' }

$autocrlf = Invoke-AutocrlfSelfTest
$goldenPaths = @(Get-ChildItem -LiteralPath (Join-Path $repo 'crates\bcsp-contracts\tests\golden') -Filter '*.json' -File | Sort-Object Name)
$goldenHashes = [ordered]@{}
foreach ($file in $goldenPaths) { $goldenHashes[$file.Name] = Get-CanonicalTextSha256 -Path $file.FullName }
if ($goldenPaths.Count -ne 9) { throw 'golden fixture cardinality changed' }

$quality = [ordered]@{
    schemaVersion = 1
    taskId = 'P7.1-004-R1'
    state = 'PASS_COMMIT_ELIGIBLE'
    primaryCommit = $primaryCommit
    primaryCommitPaths = 41
    repairAllowlistPaths = 9
    sourceContract = 'CRLF_TO_LF_ONLY_LONE_CR_REJECTED'
    wireGoldenTests = 10
    goldenCanonicalSha256 = $goldenHashes
    isolatedAutocrlfTrue = [ordered]@{
        state = [string]$autocrlf.State
        observedCrlfFiles = [int]$autocrlf.ObservedCrlfFiles
        rawBlobMismatches = [int]$autocrlf.RawBlobMismatches
        normalStatusRows = [int]$autocrlf.NormalStatusRows
    }
    dependencyDelta = [ordered]@{
        manifestsOrLocksChanged = 0
        thirdPartyPackages = $thirdParty.Count
        added = 0
        removed = 0
    }
    builderExecutedGateIds = @($gateIds)
    declaredLifecycleGateIds = @($contract.qualityGates | ForEach-Object { [string]$_ })
    remainingPostCommitOrPostPushGates = @('fast-forward-remote-boundary', 'terminal-clean-clone-replay')
    zeroSideEffects = [ordered]@{
        rutgersRequests = 0
        databaseMutations = 0
        packageBuilds = 0
        vultrMutations = 0
        releasePublications = 0
        productionMutations = 0
    }
}

$completionMarkdown = @"
# P7.1-004-R1 completion record

- Task: ``P7.1-004-R1``
- State: ``P7_1_004_R1_PASS_COMMIT_ELIGIBLE``
- Immutable primary: ``$primaryCommit``
- Repair allowlist paths: ``9``
- Golden fixtures changed: ``0``
- Wire golden tests: ``10 passed``
- Isolated ``core.autocrlf=true`` checkout: ``PASS``
- Cargo/npm dependency and lock delta: ``0``
- Third-party closure delta: ``0``

The repair canonicalizes CRLF to LF only inside the golden comparison, rejects lone carriage returns, and preserves all other exact wire bytes. The full locked/offline workspace, architecture, policy, focused contract and domain gates passed. No Rutgers request, database mutation, package build, Vultr mutation, release publication or production mutation occurred.

``P7.1-005`` remains blocked until ordinary PreCommit, the independent repair commit, PostCommit, ordinary push, shared-worktree PostPush, and a fresh remote ``core.autocrlf=true`` clone emit ``P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY``.
"@
$completionMarkdown = $completionMarkdown.Replace("`r`n", "`n").TrimStart() + "`n"

if ($VerifyOnly) {
    $scratch = Join-Path $scratchRoot ("verify-" + [Guid]::NewGuid().ToString('N'))
    [void](New-Item -ItemType Directory -Path $scratch -Force)
    $outputEvidence = Join-Path $scratch 'evidence'
    $outputRecords = Join-Path $scratch 'records'
}
else {
    $outputEvidence = $evidenceRoot
    $outputRecords = $recordsRoot
}

try {
    Write-Json -Path (Join-Path $outputEvidence 'repair-quality.json') -Value $quality
    Write-Utf8 -Path (Join-Path $outputRecords 'p7-1-004-r1-completion.md') -Content $completionMarkdown
    $completion = [ordered]@{
        schemaVersion = 1
        recordId = 'P7-1-004-R1-COMPLETION-2026-07-13-001'
        taskId = 'P7.1-004-R1'
        state = 'P7_1_004_R1_PASS_COMMIT_ELIGIBLE'
        branch = $expectedBranch
        expectedParent = $primaryCommit
        commitAllowlist = $allowlist
        commitAllowlistPaths = 9
        primaryCommitPaths = 41
        topology = 'DIRECT_SINGLE_PARENT_FAST_FORWARD_CHILD'
        sourceContract = 'CRLF_TO_LF_ONLY_LONE_CR_REJECTED'
        wireGoldenTests = 10
        isolatedAutocrlfCloneState = 'PASS'
        evidenceSha256 = [ordered]@{
            'commit-allowlist.txt' = Get-CanonicalTextSha256 -Path (Join-Path $evidenceRoot 'commit-allowlist.txt')
            'repair-quality.json' = Get-CanonicalTextSha256 -Path (Join-Path $outputEvidence 'repair-quality.json')
        }
        completionMarkdownSha256 = Get-CanonicalTextSha256 -Path (Join-Path $outputRecords 'p7-1-004-r1-completion.md')
        actualCommitExcludedToAvoidSelfReference = $true
        negativeSideEffects = $quality.zeroSideEffects
        nextTask = 'P7.1-005'
        nextTaskBlockedUntil = 'P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY'
    }
    Write-Json -Path (Join-Path $outputRecords 'p7-1-004-r1-completion.json') -Value $completion

    if ($VerifyOnly) {
        foreach ($relative in @('evidence\p7-1-004-r1\repair-quality.json', 'records\p7-1-004-r1-completion.md', 'records\p7-1-004-r1-completion.json')) {
            $actual = Join-Path $p7 $relative
            $expected = if ($relative.StartsWith('evidence')) { Join-Path $outputEvidence 'repair-quality.json' } else { Join-Path $outputRecords ([System.IO.Path]::GetFileName($relative)) }
            if (-not (Test-Path -LiteralPath $actual -PathType Leaf) -or (Get-CanonicalTextSha256 -Path $actual) -cne (Get-CanonicalTextSha256 -Path $expected)) {
                throw "VerifyOnly generated evidence differs: $relative"
            }
        }
    }
}
finally {
    if ($VerifyOnly -and $null -ne $scratch) { Remove-SafeScratch -Path $scratch }
}

Test-RepairBoundary -Allowlist $allowlist -RequiredGovernancePaths @($contract.commitBoundary.requiredGovernancePaths | ForEach-Object { [string]$_ })
$profile = Test-ProtectedWorktreeProfile -Allowlist $allowlist
Test-PublicationFiles -Paths $allowlist

$effectiveAutocrlf = (Invoke-Git -Arguments @('config', '--get', 'core.autocrlf') -AllowFailure $true).Output -join ''
$goldenRelativePaths = @($goldenPaths | ForEach-Object { Normalize-Path $_.FullName.Substring($repo.Length + 1) })
$observedCrlf = @((Invoke-Git -Arguments (@('ls-files', '--eol', '--') + $goldenRelativePaths)).Output | Where-Object { $_ -match 'i/lf\s+w/crlf' }).Count
$rawBlobMismatchFiles = 0
foreach ($path in $goldenRelativePaths) {
    $blob = Get-GitSingle -Arguments @('rev-parse', "HEAD:$path")
    $rawWorktree = Get-GitSingle -Arguments @('hash-object', '--no-filters', '--', $path)
    if ($blob -cne $rawWorktree) { $rawBlobMismatchFiles++ }
}
if ($RequireRemoteCleanReplay) {
    if ([string]$profile.Profile -cne 'CLEAN_CHECKOUT_0' -or [int]$profile.Rows -ne 0) { throw 'remote clean replay requires CLEAN_CHECKOUT_0' }
    if ($effectiveAutocrlf -cne 'true') { throw 'remote clean replay requires effective core.autocrlf=true' }
    if ($observedCrlf -lt 1) { throw 'remote clean replay did not observe CRLF golden checkout files' }
    if ($rawBlobMismatchFiles -lt 1) { throw 'remote clean replay did not observe raw LF-blob/CRLF-worktree divergence' }
    $reflog = @((Invoke-Git -Arguments @('reflog', '--format=%H', 'HEAD')).Output | Where-Object { $_ -ne '' })
    if ($reflog.Count -ne 1) { throw 'remote clean replay requires a fresh one-entry HEAD reflog' }
    $reflogSubject = Get-GitSingle -Arguments @('reflog', '-1', '--format=%gs', 'HEAD')
    if ($reflogSubject -cne "clone: from $expectedRemote") { throw 'remote clean replay HEAD reflog is not the expected remote clone entry' }
}

Write-Output "p7_1_004_r1_evidence=$(if ($VerifyOnly) { 'VERIFIED' } else { 'WRITTEN' })"
Write-Output 'wire_golden_tests=PASS_10'
Write-Output 'isolated_autocrlf_true=PASS'
Write-Output "protected_worktree_profile=$([string]$profile.Profile)"
Write-Output "protected_worktree_rows=$([int]$profile.Rows)"
Write-Output "checkout_core_autocrlf=$effectiveAutocrlf"
Write-Output "observed_crlf_golden_files=$observedCrlf"
Write-Output "raw_blob_mismatch_golden_files=$rawBlobMismatchFiles"
Write-Output "remote_clean_replay=$(if ($RequireRemoteCleanReplay) { 'PASS' } else { 'NOT_REQUIRED' })"
Write-Output 'dependency_lock_delta=ZERO'
Write-Output 'third_party_closure_delta=ZERO'
Write-Output 'rutgers_requests=0'
Write-Output 'database_mutations=0'
Write-Output 'package_builds=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_004_r1_gate=PASS'
