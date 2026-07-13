[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('PreCommit', 'PostCommit', 'PostPush')]
    [string]$Mode
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '04a-p7-1-004-shared-domain-identity-and-typed-api-schema.json'
$allowlistPath = Join-Path $p7 'evidence\p7-1-004\commit-allowlist.txt'
$proofPath = Join-Path $p7 'evidence\p7-1-004\predecessor-r1-clean-replay.json'
$qualityPath = Join-Path $p7 'evidence\p7-1-004\quality-gates.json'
$completionPath = Join-Path $p7 'records\p7-1-004-completion.json'
$completionMarkdownPath = Join-Path $p7 'records\p7-1-004-completion.md'
$builderPath = Join-Path $p7 'tools\build-p7-1-004-evidence.ps1'
$expectedParent = '870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38'
$expectedBranch = 'codex/p7-implementation'
$expectedRemote = 'https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
$expectedPredecessorState = 'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY'

function Normalize-Path([string]$Value) {
    return $Value.Replace('\', '/')
}

function Read-Json([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw 'required JSON evidence is missing'
    }
    return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json
}

function Invoke-Git([string[]]$Arguments, [bool]$AllowFailure = $false) {
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& git -C $repo @Arguments 2>$null | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if (-not $AllowFailure -and $exitCode -ne 0) {
        throw 'required Git operation failed'
    }
    return [pscustomobject]@{ Output = $output; ExitCode = $exitCode }
}

function Get-GitSingle([string[]]$Arguments) {
    $result = Invoke-Git -Arguments $Arguments
    if (@($result.Output).Count -ne 1 -or [string]::IsNullOrWhiteSpace([string]$result.Output[0])) {
        throw 'Git identity operation did not return exactly one value'
    }
    return ([string]$result.Output[0]).Trim()
}

function Get-LiveRemoteHead {
    $result = Invoke-Git -Arguments @('ls-remote', '--heads', '--exit-code', $expectedRemote, "refs/heads/$expectedBranch")
    if (@($result.Output).Count -ne 1) {
        throw 'live remote branch did not resolve to exactly one row'
    }
    $fields = @(([string]$result.Output[0]) -split "`t", 2)
    if ($fields.Count -ne 2 -or $fields[0] -notmatch '^[0-9a-f]{40}$' -or $fields[1] -cne "refs/heads/$expectedBranch") {
        throw 'live remote branch row is malformed'
    }
    return [string]$fields[0]
}

function Compare-Set([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    $actualSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $expectedSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($item in @($Actual)) {
        if (-not $actualSet.Add((Normalize-Path $item))) {
            throw "$Label contains a duplicate row"
        }
    }
    foreach ($item in @($Expected)) {
        if (-not $expectedSet.Add((Normalize-Path $item))) {
            throw "$Label expected set contains a duplicate row"
        }
    }
    if (-not $actualSet.SetEquals($expectedSet)) {
        throw "$Label set mismatch"
    }
}

function Get-IndexPaths {
    return @(
        (Invoke-Git -Arguments @('diff', '--cached', '--name-only', '--no-renames')).Output |
            Where-Object { $_ -ne '' } |
            ForEach-Object { Normalize-Path ([string]$_) }
    )
}

function Get-CommitPaths([string]$Revision) {
    return @(
        (Invoke-Git -Arguments @('diff-tree', '--no-renames', '--no-commit-id', '--name-only', '-r', $Revision)).Output |
            Where-Object { $_ -ne '' } |
            ForEach-Object { Normalize-Path ([string]$_) }
    )
}

function Get-CanonicalTextSha256([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $utf8 = [Text.UTF8Encoding]::new($false, $true)
    $text = $utf8.GetString($bytes)
    if (($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) -or $text.Contains([char]0) -or $text.Replace("`r`n", '').Contains("`r")) {
        throw 'governed text encoding is not canonicalizable UTF-8'
    }
    $canonical = $text.Replace("`r`n", "`n")
    if (-not $canonical.EndsWith("`n")) {
        throw 'governed text lacks a final newline'
    }
    $hasher = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $hasher.ComputeHash([Text.UTF8Encoding]::new($false).GetBytes($canonical))
        return -join ($hash | ForEach-Object { $_.ToString('X2') })
    }
    finally {
        $hasher.Dispose()
    }
}

function Test-DeniedPath([string]$Path) {
    $normalized = Normalize-Path $Path
    return [IO.Path]::IsPathRooted($normalized) -or
        $normalized -match '^[A-Za-z]:' -or
        $normalized -match '(^|/)\.\.(/|$)' -or
        $normalized -match '(?i)(^|/)\.secrets(/|$)' -or
        $normalized -match '(?i)(^|/)chat-log-[^/]*\.md$' -or
        $normalized -match '(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)' -or
        $normalized -match '(?i)^project-governance/current/p1/' -or
        $normalized -match '(?i)(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)' -or
        $normalized -match '(?i)(\.sqlite3?|\.db|-wal|-shm)$'
}

function Test-PublicationText([string]$Content) {
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
        if ([regex]::IsMatch($Content, $pattern)) {
            throw 'publication safety scan failed'
        }
    }
}

function Test-PublicationBlobs([string]$Source, [string[]]$Paths) {
    foreach ($path in @($Paths)) {
        if (Test-DeniedPath -Path $path) {
            throw 'denied path entered the publication boundary'
        }
        $spec = if ($Source -ceq 'index') { ":$path" } else { "$Source`:$path" }
        $exists = Invoke-Git -Arguments @('cat-file', '-e', $spec) -AllowFailure $true
        if ($exists.ExitCode -ne 0) {
            continue
        }
        $blob = Invoke-Git -Arguments @('show', $spec)
        Test-PublicationText -Content ((@($blob.Output) -join "`n") + "`n")
    }
}

function Invoke-BuilderVerifyOnly {
    $powershellExe = Join-Path $PSHOME 'powershell.exe'
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $powershellExe -NoProfile -ExecutionPolicy Bypass -File $builderPath -VerifyOnly 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw 'P7.1-004 evidence VerifyOnly failed'
    }
    $values = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach ($line in $output) {
        if ($line -notmatch '^([a-z0-9_]+)=(.*)$' -or $values.ContainsKey([string]$matches[1])) {
            throw 'P7.1-004 evidence VerifyOnly output is not a unique key/value schema'
        }
        $values.Add([string]$matches[1], [string]$matches[2])
    }
    $expected = [ordered]@{
        p7_1_004_evidence = 'VERIFIED'
        predecessor_replay = 'NOT_INVOKED'
        domain_api_contract = 'PASS'
        dependency_lock_delta = 'ZERO'
        quality_gates = 'PASS'
        protected_worktree_profile = $null
        protected_worktree_rows = $null
    }
    if ($values.Count -ne $expected.Count) {
        throw 'P7.1-004 evidence VerifyOnly output key count mismatch'
    }
    foreach ($entry in $expected.GetEnumerator()) {
        if (-not $values.ContainsKey([string]$entry.Key)) {
            throw 'P7.1-004 evidence VerifyOnly output key set mismatch'
        }
        if ($null -ne $entry.Value -and $values[[string]$entry.Key] -cne [string]$entry.Value) {
            throw 'P7.1-004 evidence VerifyOnly output value mismatch'
        }
    }
    if ($values['protected_worktree_profile'] -notin @('EXACT_PRESERVED_167', 'CLEAN_CHECKOUT_0')) {
        throw 'P7.1-004 protected worktree profile is invalid'
    }
    $rows = 0
    if ($values['protected_worktree_rows'] -notmatch '^[0-9]+$' -or -not [int]::TryParse($values['protected_worktree_rows'], [ref]$rows)) {
        throw 'P7.1-004 protected worktree row count is invalid'
    }
    if (($values['protected_worktree_profile'] -ceq 'EXACT_PRESERVED_167' -and $rows -ne 167) -or ($values['protected_worktree_profile'] -ceq 'CLEAN_CHECKOUT_0' -and $rows -ne 0)) {
        throw 'P7.1-004 protected worktree profile/count mismatch'
    }
    return [pscustomobject]@{ Profile = $values['protected_worktree_profile']; Rows = $rows }
}

$contract = Read-Json -Path $contractPath
if ([int]$contract.schemaVersion -ne 1 -or [string]$contract.taskId -cne 'P7.1-004' -or [string]$contract.state -cne 'IMPLEMENTATION_CONTRACT' -or [string]$contract.branch -cne $expectedBranch -or [string]$contract.expectedParent -cne $expectedParent) {
    throw 'P7.1-004 contract identity mismatch'
}
if ([bool]$contract.dependency.postCommitReplayAllowed -or [bool]$contract.dependency.postPushReplayAllowed -or [int]$contract.predecessorReplay.verifyOnlyHelperInvocationCount -ne 0 -or [int]$contract.predecessorReplay.postCommitHelperInvocationCount -ne 0 -or [int]$contract.predecessorReplay.postPushHelperInvocationCount -ne 0) {
    throw 'P7.1-004 predecessor replay phase boundary mismatch'
}
$allowlist = @($contract.commitBoundary.taskCommitAllowlist | ForEach-Object { Normalize-Path ([string]$_) })
if (-not [bool]$contract.commitBoundary.finalized -or $allowlist.Count -ne 41 -or [int]$contract.commitBoundary.expectedTaskPathCount -ne 41) {
    throw 'P7.1-004 commit boundary is not finalized at 41 paths'
}
$sorted = [string[]]$allowlist.Clone()
[Array]::Sort($sorted, [StringComparer]::Ordinal)
for ($index = 0; $index -lt $allowlist.Count; $index++) {
    if ([string]$allowlist[$index] -cne [string]$sorted[$index] -or (Test-DeniedPath -Path $allowlist[$index])) {
        throw 'P7.1-004 allowlist ordering or path policy mismatch'
    }
}
Compare-Set -Actual $allowlist -Expected @($allowlist | Sort-Object -Unique) -Label 'contract allowlist uniqueness'

$checkedInAllowlist = @(Get-Content -LiteralPath $allowlistPath -Encoding utf8 | Where-Object { $_ -ne '' } | ForEach-Object { Normalize-Path ([string]$_) })
Compare-Set -Actual $checkedInAllowlist -Expected $allowlist -Label 'checked-in allowlist'

$proof = Read-Json -Path $proofPath
if ([int]$proof.schemaVersion -ne 1 -or [string]$proof.taskId -cne 'P7.1-004' -or [string]$proof.state -cne 'PASS' -or [string]$proof.predecessorTask -cne 'P7.1-003-R1' -or [string]$proof.requiredCommit -cne $expectedParent -or [string]$proof.requiredTerminalState -cne $expectedPredecessorState -or -not [bool]$proof.equivalent) {
    throw 'P7.1-004 predecessor combined proof mismatch'
}
if ([string]$proof.entryCapture.purpose -cne 'EntryCapture' -or [string]$proof.preCommitReplay.purpose -cne 'PreCommitReplay' -or [bool]$proof.replayPolicy.verifyOnlyInvokesHelper -or [bool]$proof.replayPolicy.postCommitInvokesHelper -or [bool]$proof.replayPolicy.postPushInvokesHelper) {
    throw 'P7.1-004 predecessor proof invocation policy mismatch'
}

$quality = Read-Json -Path $qualityPath
$completion = Read-Json -Path $completionPath
if ([string]$quality.taskId -cne 'P7.1-004' -or [string]$quality.state -cne 'PASS' -or @($quality.gates).Count -ne 25 -or [bool]$quality.rawCommandOutputPublished -or [bool]$quality.absoluteLocalPathsPublished -or [bool]$quality.sensitiveMaterialPublished) {
    throw 'P7.1-004 quality evidence mismatch'
}
if ([string]$completion.taskId -cne 'P7.1-004' -or [string]$completion.state -cne 'P7_1_004_PASS_COMMIT_ELIGIBLE' -or [string]$completion.expectedParent -cne $expectedParent -or [int]$completion.commitAllowlistPaths -ne 41 -or -not [bool]$completion.actualCommitExcludedToAvoidSelfReference -or [string]$completion.nextTask -cne 'P7.1-005' -or [string]$completion.nextTaskBlockedUntil -cne 'P7_1_004_PASS_POST_PUSH') {
    throw 'P7.1-004 completion record mismatch'
}
Compare-Set -Actual @($completion.commitAllowlist | ForEach-Object { [string]$_ }) -Expected $allowlist -Label 'completion allowlist'
foreach ($name in @('commit-allowlist.txt', 'dependency-closure-delta.json', 'domain-api-contract.json', 'predecessor-r1-clean-replay.json', 'quality-gates.json')) {
    $evidencePath = Join-Path $p7 "evidence\p7-1-004\$name"
    if ([string]$completion.evidenceSha256.$name -cne (Get-CanonicalTextSha256 -Path $evidencePath)) {
        throw 'P7.1-004 evidence hash mismatch'
    }
}
if ([string]$completion.contractCanonicalTextSha256 -cne (Get-CanonicalTextSha256 -Path $contractPath) -or [string]$completion.completionMarkdownSha256 -cne (Get-CanonicalTextSha256 -Path $completionMarkdownPath)) {
    throw 'P7.1-004 contract/completion canonical hash mismatch'
}
foreach ($name in @('rutgersRequests', 'databaseMutations', 'packageBuilds', 'vultrMutations', 'releasePublications', 'productionMutations')) {
    if ([int]$completion.negativeSideEffects.$name -ne 0) {
        throw 'P7.1-004 completion reports a forbidden side effect'
    }
}

$branch = Get-GitSingle -Arguments @('branch', '--show-current')
$head = Get-GitSingle -Arguments @('rev-parse', 'HEAD')
$origin = Get-GitSingle -Arguments @('remote', 'get-url', 'origin')
$tracking = Get-GitSingle -Arguments @('rev-parse', "refs/remotes/origin/$expectedBranch")
$live = Get-LiveRemoteHead
if ($branch -cne $expectedBranch -or $origin -cne $expectedRemote) {
    throw 'P7.1-004 branch/origin identity mismatch'
}

if ($Mode -ceq 'PreCommit') {
    if ($head -cne $expectedParent -or $tracking -cne $expectedParent -or $live -cne $expectedParent) {
        throw 'P7.1-004 PreCommit remote boundary mismatch'
    }
    Compare-Set -Actual @(Get-IndexPaths) -Expected $allowlist -Label 'PreCommit staged allowlist'
    $unstaged = @((Invoke-Git -Arguments (@('diff', '--name-only', '--') + $allowlist)).Output | Where-Object { $_ -ne '' })
    if ($unstaged.Count -ne 0 -or @((Invoke-Git -Arguments @('diff', '--cached', '--check') -AllowFailure $true).Output).Count -ne 0) {
        throw 'P7.1-004 PreCommit task diff is not fully staged and clean'
    }
}
else {
    $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', 'HEAD')) -split '\s+'
    if ($parents.Count -ne 2 -or $parents[0] -cne $head -or $parents[1] -cne $expectedParent) {
        throw 'P7.1-004 commit is not the direct single-parent predecessor child'
    }
    Compare-Set -Actual @(Get-CommitPaths -Revision 'HEAD') -Expected $allowlist -Label 'committed allowlist'
    if (@(Get-IndexPaths).Count -ne 0) {
        throw 'P7.1-004 committed index is not empty'
    }
    $taskDirty = @((Invoke-Git -Arguments (@('diff', '--name-only', '--') + $allowlist)).Output | Where-Object { $_ -ne '' })
    if ($taskDirty.Count -ne 0 -or @((Invoke-Git -Arguments @('diff-tree', '--check', 'HEAD^', 'HEAD') -AllowFailure $true).Output).Count -ne 0) {
        throw 'P7.1-004 committed task paths are dirty or malformed'
    }
    if ($Mode -ceq 'PostCommit') {
        if ($tracking -cne $expectedParent -or $live -cne $expectedParent) {
            throw 'P7.1-004 remote advanced before PostCommit completed'
        }
    }
    else {
        $upstream = Get-GitSingle -Arguments @('rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{upstream}')
        if ($tracking -cne $head -or $live -cne $head -or $upstream -cne "origin/$expectedBranch") {
            throw 'P7.1-004 PostPush remote/upstream boundary mismatch'
        }
    }
}

$profile = Invoke-BuilderVerifyOnly
if ($Mode -ceq 'PreCommit') {
    Test-PublicationBlobs -Source 'index' -Paths $allowlist
}
else {
    Test-PublicationBlobs -Source 'HEAD' -Paths $allowlist
    $message = (@((Invoke-Git -Arguments @('log', '-1', '--format=%B', 'HEAD')).Output) -join "`n") + "`n"
    Test-PublicationText -Content $message
}

$state = switch ($Mode) {
    'PreCommit' { 'P7_1_004_PASS_PRE_COMMIT' }
    'PostCommit' { 'P7_1_004_PASS_POST_COMMIT' }
    'PostPush' { 'P7_1_004_PASS_POST_PUSH' }
}
Write-Output "validation_mode=$Mode"
Write-Output "validation_state=$state"
Write-Output 'active_task=P7.1-004'
Write-Output 'task_allowlist_paths=41'
Write-Output "protected_worktree_profile=$($profile.Profile)"
Write-Output "protected_worktree_rows=$($profile.Rows)"
Write-Output 'predecessor_replay=COMMITTED_PROOF_ONLY_NO_HELPER'
Write-Output 'rutgers_requests=0'
Write-Output 'database_mutations=0'
Write-Output 'package_builds=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_004_gate=PASS'
