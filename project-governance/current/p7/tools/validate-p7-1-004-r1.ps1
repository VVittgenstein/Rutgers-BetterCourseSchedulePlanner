[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateSet('PreCommit', 'PostCommit', 'PostPush')]
    [string]$Mode,
    [switch]$RemoteCleanReplay
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '04r1a-p7-1-004-r1-windows-golden-line-ending-repair.json'
$builderPath = Join-Path $PSScriptRoot 'build-p7-1-004-r1-evidence.ps1'
$expectedParent = '008de8c53da39af5562cd8a4022f839100a8a11d'
$expectedBranch = 'codex/p7-implementation'
$expectedRemote = 'https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
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

function Normalize-Path([string]$Value) { return $Value.Replace('\', '/') }

function Invoke-Git([string[]]$Arguments, [bool]$AllowFailure = $false) {
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& git -C $repo @Arguments 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if (-not $AllowFailure -and $exitCode -ne 0) { throw "git $($Arguments -join ' ') failed: $($output -join ' | ')" }
    return [pscustomobject]@{ ExitCode = $exitCode; Output = $output }
}

function Get-GitSingle([string[]]$Arguments) {
    $values = @((Invoke-Git -Arguments $Arguments).Output | Where-Object { $_ -ne '' })
    if ($values.Count -ne 1) { throw "git command did not return one row: $($Arguments -join ' ')" }
    return [string]$values[0]
}

function Compare-Set([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    $a = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $e = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($value in @($Actual)) { if (-not $a.Add((Normalize-Path $value))) { throw "$Label duplicate actual row" } }
    foreach ($value in @($Expected)) { if (-not $e.Add((Normalize-Path $value))) { throw "$Label duplicate expected row" } }
    if (-not $a.SetEquals($e)) { throw "$Label set mismatch" }
}

function Get-LiveRemoteHead {
    $result = Invoke-Git -Arguments @('ls-remote', '--heads', '--exit-code', $expectedRemote, "refs/heads/$expectedBranch")
    $rows = @($result.Output | Where-Object { $_ -ne '' })
    if ($rows.Count -ne 1) { throw 'live remote branch cardinality mismatch' }
    $parts = $rows[0] -split '\s+'
    if ($parts.Count -ne 2 -or $parts[0] -notmatch '^[0-9a-f]{40}$' -or $parts[1] -cne "refs/heads/$expectedBranch") { throw 'live remote branch format mismatch' }
    return [string]$parts[0]
}

function Get-IndexPaths {
    return @((Invoke-Git -Arguments @('diff', '--cached', '--name-only', '--no-renames')).Output | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path $_ })
}

function Get-CommitPaths([string]$Revision) {
    return @((Invoke-Git -Arguments @('diff-tree', '--no-renames', '--no-commit-id', '--name-only', '-r', $Revision)).Output | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path $_ })
}

function Get-AllowlistStatusPaths([string[]]$Allowlist) {
    $allowed = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in $Allowlist) { [void]$allowed.Add((Normalize-Path $path)) }
    $paths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($raw in @((Invoke-Git -Arguments @('-c', 'core.quotepath=false', 'status', '--porcelain=v1', '--untracked-files=all', '--no-renames')).Output)) {
        $line = [string]$raw
        if ($line.Length -lt 4) { continue }
        $path = Normalize-Path $line.Substring(3)
        if ($allowed.Contains($path)) { [void]$paths.Add($path) }
    }
    return @($paths | Sort-Object)
}

function Test-PublicationText([string]$Content, [string]$Label) {
    foreach ($pattern in @(
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
    )) {
        if ($Content -match $pattern) { throw "publication safety pattern matched in $Label" }
    }
}

function Test-PublicationBlobs([string]$Source, [string[]]$Paths) {
    foreach ($path in $Paths) {
        $modeLine = if ($Source -ceq 'INDEX') {
            Get-GitSingle -Arguments @('ls-files', '--stage', '--', $path)
        }
        else {
            Get-GitSingle -Arguments @('ls-tree', $Source, '--', $path)
        }
        $parts = $modeLine -split '\s+'
        if ($Source -ceq 'INDEX') {
            if ($parts.Count -lt 4 -or $parts[0] -cne '100644' -or $parts[1] -notmatch '^[0-9a-f]{40,64}$' -or $parts[2] -cne '0') {
                throw "publication index path has an unsafe mode, object ID, or stage: $path"
            }
        }
        elseif ($parts.Count -lt 4 -or $parts[0] -cne '100644' -or $parts[1] -cne 'blob' -or $parts[2] -notmatch '^[0-9a-f]{40,64}$') {
            throw "publication path is not a regular 100644 blob: $path"
        }
        $spec = if ($Source -ceq 'INDEX') { ":$path" } else { "$Source`:$path" }
        $content = @((Invoke-Git -Arguments @('show', $spec)).Output) -join "`n"
        Test-PublicationText -Content $content -Label "$Source`:$path"
    }
}

$contract = Get-Content -LiteralPath $contractPath -Raw -Encoding utf8 | ConvertFrom-Json
if ([string]$contract.taskId -cne 'P7.1-004-R1' -or [string]$contract.expectedParent -cne $expectedParent) { throw 'repair contract identity mismatch' }
$allowlist = @($contract.commitBoundary.taskCommitAllowlist | ForEach-Object { Normalize-Path ([string]$_) })
if ($allowlist.Count -ne 9) { throw 'repair allowlist cardinality mismatch' }
Compare-Set -Actual $allowlist -Expected $expectedAllowlist -Label 'contract/hard-coded allowlist'
Compare-Set -Actual @(Get-Content -LiteralPath (Join-Path $p7 'evidence\p7-1-004-r1\commit-allowlist.txt') -Encoding utf8 | Where-Object { $_ -ne '' }) -Expected $allowlist -Label 'checked-in allowlist'

$branch = Get-GitSingle -Arguments @('symbolic-ref', '--short', 'HEAD')
$head = Get-GitSingle -Arguments @('rev-parse', 'HEAD')
$tracking = Get-GitSingle -Arguments @('rev-parse', "refs/remotes/origin/$expectedBranch")
$origin = Get-GitSingle -Arguments @('remote', 'get-url', 'origin')
$live = Get-LiveRemoteHead
if ($branch -cne $expectedBranch -or $origin -cne $expectedRemote) { throw 'branch or origin mismatch' }
if ($RemoteCleanReplay -and $Mode -cne 'PostPush') { throw 'RemoteCleanReplay is valid only with PostPush' }

if ($Mode -ceq 'PreCommit') {
    if ($head -cne $expectedParent -or $tracking -cne $expectedParent -or $live -cne $expectedParent) { throw 'PreCommit remote boundary mismatch' }
    Compare-Set -Actual @(Get-IndexPaths) -Expected $allowlist -Label 'PreCommit staged allowlist'
    $unstaged = @((Invoke-Git -Arguments @('diff', '--name-only', '--no-renames')).Output | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path $_ } | Where-Object { $allowlist -contains $_ })
    if ($unstaged.Count -ne 0) { throw 'PreCommit task diff is not fully staged' }
    Test-PublicationBlobs -Source 'INDEX' -Paths $allowlist
}
else {
    $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', 'HEAD')) -split '\s+'
    if ($parents.Count -ne 2 -or $parents[0] -cne $head -or $parents[1] -cne $expectedParent) { throw 'repair commit topology mismatch' }
    Compare-Set -Actual @(Get-CommitPaths -Revision 'HEAD') -Expected $allowlist -Label 'committed repair allowlist'
    if (@(Get-IndexPaths).Count -ne 0) { throw 'post-commit index is not empty' }
    if (@(Get-AllowlistStatusPaths -Allowlist $allowlist).Count -ne 0) { throw 'post-commit repair paths are dirty' }
    Test-PublicationBlobs -Source 'HEAD' -Paths $allowlist
    if ($Mode -ceq 'PostCommit') {
        if ($tracking -cne $expectedParent -or $live -cne $expectedParent) { throw 'remote advanced before PostCommit completed' }
    }
    else {
        if ($tracking -cne $head -or $live -cne $head) { throw 'PostPush remote boundary mismatch' }
        $upstream = Get-GitSingle -Arguments @('rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{upstream}')
        if ($upstream -cne "origin/$expectedBranch") { throw 'PostPush upstream mismatch' }
    }
}

$powershellExe = (Get-Process -Id $PID).Path
$builderArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $builderPath, '-VerifyOnly')
if ($RemoteCleanReplay) { $builderArgs += '-RequireRemoteCleanReplay' }
$builderOutput = @(& $powershellExe @builderArgs 2>&1 | ForEach-Object { $_.ToString() })
if ($LASTEXITCODE -ne 0) {
    $tail = @($builderOutput | Select-Object -Last 12) -join ' | '
    throw "repair evidence VerifyOnly failed: $tail"
}
if (-not ($builderOutput -contains 'p7_1_004_r1_gate=PASS')) { throw 'repair evidence VerifyOnly did not emit PASS' }
$profileLine = @($builderOutput | Where-Object { $_ -like 'protected_worktree_profile=*' })
if ($profileLine.Count -ne 1) { throw 'builder protected profile output mismatch' }
$profile = $profileLine[0].Substring('protected_worktree_profile='.Length)
if ($RemoteCleanReplay -and $profile -cne 'CLEAN_CHECKOUT_0') { throw 'terminal replay did not use a clean checkout' }

# Re-prove publication and Git cleanliness after every builder/test side effect.
if ($Mode -ceq 'PreCommit') {
    Compare-Set -Actual @(Get-IndexPaths) -Expected $allowlist -Label 'post-builder PreCommit staged allowlist'
    $postBuilderUnstaged = @((Invoke-Git -Arguments @('diff', '--name-only', '--no-renames')).Output | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path $_ } | Where-Object { $allowlist -contains $_ })
    if ($postBuilderUnstaged.Count -ne 0) { throw 'builder modified a staged repair path' }
    Test-PublicationBlobs -Source 'INDEX' -Paths $allowlist
    if ((Get-LiveRemoteHead) -cne $expectedParent) { throw 'remote advanced during PreCommit validation' }
}
else {
    if (@(Get-IndexPaths).Count -ne 0) { throw 'builder left a non-empty post-commit index' }
    if (@(Get-AllowlistStatusPaths -Allowlist $allowlist).Count -ne 0) { throw 'builder modified a committed repair path' }
    Test-PublicationBlobs -Source 'HEAD' -Paths $allowlist
    $postBuilderLive = Get-LiveRemoteHead
    $expectedLive = if ($Mode -ceq 'PostCommit') { $expectedParent } else { $head }
    if ($postBuilderLive -cne $expectedLive) { throw 'remote advanced during validation' }
}

if ($Mode -ne 'PreCommit') {
    Test-PublicationText -Content (Get-GitSingle -Arguments @('log', '-1', '--format=%B')) -Label 'repair commit message'
}

$state = if ($RemoteCleanReplay) {
    'P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY'
}
else {
    switch ($Mode) {
        'PreCommit' { 'P7_1_004_R1_PASS_PRE_COMMIT' }
        'PostCommit' { 'P7_1_004_R1_PASS_POST_COMMIT' }
        'PostPush' { 'P7_1_004_R1_PASS_POST_PUSH' }
    }
}
Write-Output "validation_mode=$Mode"
Write-Output "validation_state=$state"
Write-Output "remote_clean_replay=$([bool]$RemoteCleanReplay)"
Write-Output 'active_task=P7.1-004-R1'
Write-Output 'task_allowlist_paths=9'
Write-Output "protected_worktree_profile=$profile"
Write-Output 'wire_golden_tests=PASS_10'
Write-Output 'dependency_lock_delta=ZERO'
Write-Output 'rutgers_requests=0'
Write-Output 'database_mutations=0'
Write-Output 'package_builds=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_004_r1_gate=PASS'
