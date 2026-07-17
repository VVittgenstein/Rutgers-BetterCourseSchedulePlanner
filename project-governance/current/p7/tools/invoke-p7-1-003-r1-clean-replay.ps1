param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('EntryCapture', 'PreCommitReplay')]
    [string]$Purpose
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$expectedHead = '870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38'
$expectedBranch = 'codex/p7-implementation'
$expectedRemote = 'https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
$expectedState = 'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY'
$validatorRelative = 'project-governance/current/p7/tools/validate-p7-1-003-r1.ps1'
$expectedValidatorBlob = '5b76c162a68f44da00305d1c211b3d191aff30a3'
$script:GitExe = $null

function Stop-Replay {
    param([Parameter(Mandatory = $true)][string]$Code)

    if ($Code -notmatch '^[A-Z0-9_]+$') {
        $Code = 'INTERNAL_FAILURE'
    }
    $exception = [InvalidOperationException]::new("REPLAY_FAILURE:$Code")
    $exception.Data['ReplayFailureCode'] = $Code
    throw $exception
}

function Get-ReplayFailureCode {
    param([Parameter(Mandatory = $true)]$ErrorRecord)

    $exception = $ErrorRecord.Exception
    while ($null -ne $exception) {
        $code = [string]$exception.Data['ReplayFailureCode']
        if ($code -match '^[A-Z0-9_]+$') {
            return $code
        }
        if ([string]$exception.Message -match '^REPLAY_FAILURE:([A-Z0-9_]+)$') {
            return [string]$matches[1]
        }
        $exception = $exception.InnerException
    }
    return 'UNEXPECTED_FAILURE'
}

function Invoke-GitChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [Parameter(Mandatory = $true)]
        [string]$WorkingDirectory,
        [Parameter(Mandatory = $true)]
        [string]$FailureCode
    )

    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& $script:GitExe -C $WorkingDirectory @Arguments 2>&1 | ForEach-Object { $_.ToString() })
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        Stop-Replay -Code $FailureCode
    }
    return $output
}

function Get-SingleLine {
    param(
        [Parameter(Mandatory = $true)]
        [AllowEmptyCollection()]
        [object[]]$Lines,
        [Parameter(Mandatory = $true)]
        [string]$FailureCode,
        [switch]$AllowWhitespace
    )

    $items = @($Lines | ForEach-Object { $_.ToString() })
    if ($items.Count -ne 1) {
        Stop-Replay -Code $FailureCode
    }
    if (-not $AllowWhitespace -and [string]::IsNullOrWhiteSpace([string]$items[0])) {
        Stop-Replay -Code $FailureCode
    }
    return [string]$items[0]
}

function Get-SingleGitLine {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [Parameter(Mandatory = $true)]
        [string]$WorkingDirectory,
        [Parameter(Mandatory = $true)]
        [string]$FailureCode
    )

    $lines = @(Invoke-GitChecked -Arguments $Arguments -WorkingDirectory $WorkingDirectory -FailureCode $FailureCode)
    return Get-SingleLine -Lines $lines -FailureCode $FailureCode
}

function Get-LiveRemoteHead {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Remote,
        [Parameter(Mandatory = $true)]
        [string]$Branch,
        [Parameter(Mandatory = $true)]
        [string]$WorkingDirectory,
        [Parameter(Mandatory = $true)]
        [string]$FailureCode
    )

    $rows = @(Invoke-GitChecked -Arguments @('ls-remote', '--heads', '--exit-code', $Remote, "refs/heads/$Branch") -WorkingDirectory $WorkingDirectory -FailureCode $FailureCode)
    $row = Get-SingleLine -Lines $rows -FailureCode $FailureCode
    $fields = @($row -split "`t", 2)
    if ($fields.Count -ne 2 -or $fields[0] -notmatch '^[0-9a-f]{40}$' -or $fields[1] -cne "refs/heads/$Branch") {
        Stop-Replay -Code $FailureCode
    }
    return [string]$fields[0]
}

function Get-UpperSha256 {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)

    $hasher = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $hasher.ComputeHash($Bytes)
        return -join ($hash | ForEach-Object { $_.ToString('X2') })
    }
    finally {
        $hasher.Dispose()
    }
}

function Assert-DirectChildPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Parent,
        [Parameter(Mandatory = $true)][string]$LeafPattern,
        [Parameter(Mandatory = $true)][string]$FailureCode
    )

    $fullPath = [IO.Path]::GetFullPath($Path)
    $fullParent = [IO.Path]::GetFullPath($Parent)
    if (-not (Split-Path -Parent $fullPath).Equals($fullParent, [StringComparison]::OrdinalIgnoreCase)) {
        Stop-Replay -Code $FailureCode
    }
    if ((Split-Path -Leaf $fullPath) -notmatch $LeafPattern) {
        Stop-Replay -Code $FailureCode
    }
}

function Assert-NormalDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$FailureCode
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        Stop-Replay -Code $FailureCode
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer -or (([int]$item.Attributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0)) {
        Stop-Replay -Code $FailureCode
    }
}

function Assert-KnownJunction {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedTarget,
        [Parameter(Mandatory = $true)][string]$FailureCode
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        Stop-Replay -Code $FailureCode
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer -or (([int]$item.Attributes -band [int][IO.FileAttributes]::ReparsePoint) -eq 0) -or [string]$item.LinkType -cne 'Junction') {
        Stop-Replay -Code $FailureCode
    }
    $targets = @($item.Target)
    if ($targets.Count -ne 1) {
        Stop-Replay -Code $FailureCode
    }
    $observedTarget = [IO.Path]::GetFullPath([string]$targets[0])
    $requiredTarget = [IO.Path]::GetFullPath($ExpectedTarget)
    if (-not $observedTarget.Equals($requiredTarget, [StringComparison]::OrdinalIgnoreCase)) {
        Stop-Replay -Code $FailureCode
    }
}

function Remove-ReplayClone {
    param(
        [Parameter(Mandatory = $true)][string]$ClonePath,
        [Parameter(Mandatory = $true)][string]$CacheRoot,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$CreatedJunctions
    )

    if (-not (Test-Path -LiteralPath $ClonePath)) {
        return 'NOT_REQUIRED'
    }

    Assert-DirectChildPath -Path $ClonePath -Parent $CacheRoot -LeafPattern '^r1-[0-9a-f]{8}$' -FailureCode 'CLEANUP_PATH_BOUNDARY_FAILED'
    Assert-NormalDirectory -Path $ClonePath -FailureCode 'CLEANUP_CLONE_ROOT_REPARSE'

    foreach ($definition in @($CreatedJunctions)) {
        $junctionPath = [string]$definition.Path
        $junctionTarget = [string]$definition.Target
        Assert-KnownJunction -Path $junctionPath -ExpectedTarget $junctionTarget -FailureCode 'CLEANUP_JUNCTION_IDENTITY_FAILED'
        [IO.Directory]::Delete($junctionPath, $false)
        if (Test-Path -LiteralPath $junctionPath) {
            Stop-Replay -Code 'CLEANUP_JUNCTION_DELETE_FAILED'
        }
    }

    $extendedClone = if ($ClonePath.StartsWith('\\?\', [StringComparison]::Ordinal)) { $ClonePath } else { '\\?\' + $ClonePath }
    $stack = [System.Collections.Generic.Stack[string]]::new()
    $stack.Push($extendedClone)
    while ($stack.Count -gt 0) {
        $directory = $stack.Pop()
        foreach ($entry in [IO.Directory]::EnumerateFileSystemEntries($directory)) {
            $attributes = [IO.File]::GetAttributes($entry)
            if (([int]$attributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0) {
                Stop-Replay -Code 'CLEANUP_UNKNOWN_REPARSE_POINT'
            }
            if (([int]$attributes -band [int][IO.FileAttributes]::ReadOnly) -ne 0) {
                $cleared = [IO.FileAttributes]([int]$attributes -band (-bnot [int][IO.FileAttributes]::ReadOnly))
                [IO.File]::SetAttributes($entry, $cleared)
            }
            if (([int]$attributes -band [int][IO.FileAttributes]::Directory) -ne 0) {
                $stack.Push($entry)
            }
        }
    }

    $rootAttributes = [IO.File]::GetAttributes($extendedClone)
    if (([int]$rootAttributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0) {
        Stop-Replay -Code 'CLEANUP_CLONE_ROOT_REPARSE'
    }
    if (([int]$rootAttributes -band [int][IO.FileAttributes]::ReadOnly) -ne 0) {
        $clearedRoot = [IO.FileAttributes]([int]$rootAttributes -band (-bnot [int][IO.FileAttributes]::ReadOnly))
        [IO.File]::SetAttributes($extendedClone, $clearedRoot)
    }
    [IO.Directory]::Delete($extendedClone, $true)
    if (Test-Path -LiteralPath $ClonePath) {
        Stop-Replay -Code 'CLEANUP_RECURSIVE_DELETE_FAILED'
    }
    return 'PASS'
}

function Test-PublicJsonSafe {
    param([Parameter(Mandatory = $true)][string]$Json)

    $patterns = @(
        '-----BEGIN [A-Z ]*PRIVATE KEY-----',
        '(?i)ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}',
        'github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}',
        'npm_[A-Za-z0-9]{20,}',
        'sk-(proj-)?[A-Za-z0-9_-]{20,}',
        'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}',
        '(?i)https?://[^/\s:@]+:[^/\s@]+@',
        '(?i)(^|["\s])[A-Z]:[\\/]',
        '\\\\',
        '(?i)/(home|Users|mnt/[a-z])/',
        '(?i)\.secrets',
        '(?i)chat-log-[^"\\/\s]*\.md',
        '(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}'
    )
    foreach ($pattern in $patterns) {
        if ([regex]::IsMatch($Json, $pattern)) {
            return $false
        }
    }
    return $true
}

$failureCode = $null
$cleanupState = 'NOT_REQUIRED'
$cloneOwned = $false
$successResult = $null
$createdJunctions = [System.Collections.Generic.List[object]]::new()

try {
    $p7 = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
    $repo = [IO.Path]::GetFullPath((Join-Path $p7 '..\..\..'))
    $cacheRoot = [IO.Path]::GetFullPath((Join-Path $repo '.cache'))
    $clone = Join-Path $cacheRoot ('r1-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))

    Assert-NormalDirectory -Path $repo -FailureCode 'REPOSITORY_ROOT_INVALID'
    Assert-NormalDirectory -Path $cacheRoot -FailureCode 'CACHE_ROOT_INVALID'
    Assert-DirectChildPath -Path $clone -Parent $cacheRoot -LeafPattern '^r1-[0-9a-f]{8}$' -FailureCode 'CLONE_PATH_BOUNDARY_FAILED'
    if (Test-Path -LiteralPath $clone) {
        Stop-Replay -Code 'CLONE_PATH_COLLISION'
    }

    $gitCommands = @(Get-Command git.exe -CommandType Application -ErrorAction SilentlyContinue)
    if ($gitCommands.Count -ne 1) {
        Stop-Replay -Code 'GIT_EXECUTABLE_UNAVAILABLE'
    }
    $script:GitExe = [string]$gitCommands[0].Source

    $remoteRows = @(Invoke-GitChecked -Arguments @('remote', 'get-url', 'origin') -WorkingDirectory $repo -FailureCode 'ORIGIN_LOOKUP_FAILED')
    $remote = Get-SingleLine -Lines $remoteRows -FailureCode 'ORIGIN_NOT_SINGLE'
    if ($remote -cne $expectedRemote -or $remote.StartsWith('-', [StringComparison]::Ordinal) -or $remote.IndexOfAny(@([char]0, [char]10, [char]13)) -ge 0) {
        Stop-Replay -Code 'ORIGIN_IDENTITY_MISMATCH'
    }

    $remoteBefore = Get-LiveRemoteHead -Remote $remote -Branch $expectedBranch -WorkingDirectory $repo -FailureCode 'REMOTE_BEFORE_INVALID'
    if ($remoteBefore -cne $expectedHead) {
        Stop-Replay -Code 'REMOTE_REPLAY_WINDOW_CLOSED'
    }

    $cloneOwned = $true
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $cloneOutput = @(& $script:GitExe -c 'core.autocrlf=true' clone --quiet --branch $expectedBranch --single-branch -- $remote $clone 2>&1 | ForEach-Object { $_.ToString() })
        $cloneExit = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($cloneExit -ne 0) {
        Stop-Replay -Code 'FRESH_CLONE_FAILED'
    }
    [void](Invoke-GitChecked -Arguments @('config', '--local', 'core.autocrlf', 'true') -WorkingDirectory $clone -FailureCode 'AUTOCRLF_PERSIST_FAILED')

    $headBeforeExecution = Get-SingleGitLine -Arguments @('rev-parse', 'HEAD') -WorkingDirectory $clone -FailureCode 'CLONE_HEAD_INVALID'
    $branchBeforeExecution = Get-SingleGitLine -Arguments @('branch', '--show-current') -WorkingDirectory $clone -FailureCode 'CLONE_BRANCH_INVALID'
    $trackingBeforeExecution = Get-SingleGitLine -Arguments @('rev-parse', "refs/remotes/origin/$expectedBranch") -WorkingDirectory $clone -FailureCode 'CLONE_TRACKING_REF_INVALID'
    $originBeforeExecution = Get-SingleGitLine -Arguments @('remote', 'get-url', 'origin') -WorkingDirectory $clone -FailureCode 'CLONE_ORIGIN_INVALID'
    $effectiveAutocrlfBeforeExecution = Get-SingleGitLine -Arguments @('config', '--get', 'core.autocrlf') -WorkingDirectory $clone -FailureCode 'CLONE_AUTOCRLF_INVALID'
    $statusBeforeExecution = @(Invoke-GitChecked -Arguments @('status', '--porcelain=v1', '--untracked-files=all', '--no-renames') -WorkingDirectory $clone -FailureCode 'CLONE_STATUS_FAILED')
    $reflogBeforeExecution = @(Invoke-GitChecked -Arguments @('reflog', 'show', '--format=%H%x09%gs', 'HEAD') -WorkingDirectory $clone -FailureCode 'CLONE_REFLOG_FAILED')
    $reflogLine = Get-SingleLine -Lines $reflogBeforeExecution -FailureCode 'CLONE_REFLOG_NOT_FRESH'
    $reflogFields = @($reflogLine -split "`t", 2)
    if ($headBeforeExecution -cne $expectedHead -or $branchBeforeExecution -cne $expectedBranch -or $trackingBeforeExecution -cne $expectedHead -or $originBeforeExecution -cne $expectedRemote -or $effectiveAutocrlfBeforeExecution -cne 'true' -or $statusBeforeExecution.Count -ne 0 -or $reflogFields.Count -ne 2 -or $reflogFields[0] -cne $expectedHead -or $reflogFields[1] -notlike 'clone: from *') {
        Stop-Replay -Code 'PRE_EXECUTION_IDENTITY_MISMATCH'
    }

    $validatorBlob = Get-SingleGitLine -Arguments @('rev-parse', "HEAD:$validatorRelative") -WorkingDirectory $clone -FailureCode 'VALIDATOR_BLOB_LOOKUP_FAILED'
    $validatorTree = Get-SingleGitLine -Arguments @('ls-tree', 'HEAD', '--', $validatorRelative) -WorkingDirectory $clone -FailureCode 'VALIDATOR_TREE_LOOKUP_FAILED'
    $expectedTree = "100644 blob $expectedValidatorBlob`t$validatorRelative"
    $validatorPath = Join-Path $clone $validatorRelative.Replace('/', '\')
    if ($validatorBlob -cne $expectedValidatorBlob -or $validatorTree -cne $expectedTree -or -not (Test-Path -LiteralPath $validatorPath -PathType Leaf)) {
        Stop-Replay -Code 'VALIDATOR_SOURCE_IDENTITY_MISMATCH'
    }
    $validatorItem = Get-Item -LiteralPath $validatorPath -Force
    if (([int]$validatorItem.Attributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0) {
        Stop-Replay -Code 'VALIDATOR_SOURCE_REPARSE_POINT'
    }

    $cloneCache = Join-Path $clone '.cache'
    New-Item -ItemType Directory -Path $cloneCache -Force | Out-Null
    Assert-DirectChildPath -Path $cloneCache -Parent $clone -LeafPattern '^\.cache$' -FailureCode 'CLONE_CACHE_BOUNDARY_FAILED'
    Assert-NormalDirectory -Path $cloneCache -FailureCode 'CLONE_CACHE_INVALID'
    $junctionDefinitions = @(
        [pscustomobject]@{ Path = Join-Path $cloneCache 'p7-1-002'; Target = Join-Path $cacheRoot 'p7-1-002'; Leaf = 'p7-1-002' },
        [pscustomobject]@{ Path = Join-Path $cloneCache 'p7-1-002-node-24.18.0-win-x64'; Target = Join-Path $cacheRoot 'p7-1-002-node-24.18.0-win-x64'; Leaf = 'p7-1-002-node-24.18.0-win-x64' }
    )
    foreach ($definition in $junctionDefinitions) {
        Assert-DirectChildPath -Path $definition.Path -Parent $cloneCache -LeafPattern ('^' + [regex]::Escape([string]$definition.Leaf) + '$') -FailureCode 'JUNCTION_PATH_BOUNDARY_FAILED'
        Assert-DirectChildPath -Path $definition.Target -Parent $cacheRoot -LeafPattern ('^' + [regex]::Escape([string]$definition.Leaf) + '$') -FailureCode 'JUNCTION_TARGET_BOUNDARY_FAILED'
        Assert-NormalDirectory -Path $definition.Target -FailureCode 'JUNCTION_TARGET_INVALID'
        New-Item -ItemType Junction -Path $definition.Path -Target $definition.Target | Out-Null
        [void]$createdJunctions.Add($definition)
        Assert-KnownJunction -Path $definition.Path -ExpectedTarget $definition.Target -FailureCode 'JUNCTION_CREATION_IDENTITY_FAILED'
    }
    $primaryScratch = Join-Path $cloneCache 'p7-1-003'
    New-Item -ItemType Directory -Path $primaryScratch -Force | Out-Null
    Assert-DirectChildPath -Path $primaryScratch -Parent $cloneCache -LeafPattern '^p7-1-003$' -FailureCode 'PRIMARY_SCRATCH_BOUNDARY_FAILED'
    Assert-NormalDirectory -Path $primaryScratch -FailureCode 'PRIMARY_SCRATCH_INVALID'

    $powershellExe = Join-Path $PSHOME 'powershell.exe'
    if (-not (Test-Path -LiteralPath $powershellExe -PathType Leaf)) {
        Stop-Replay -Code 'POWERSHELL_HOST_UNAVAILABLE'
    }
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $validatorOutput = @(& $powershellExe -NoProfile -ExecutionPolicy Bypass -File $validatorPath -Mode PostPush -RemoteCleanReplay 2>&1 | ForEach-Object { $_.ToString() })
        $validatorExit = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($validatorExit -ne 0) {
        Stop-Replay -Code 'IMMUTABLE_VALIDATOR_FAILED'
    }

    $expectedValues = [ordered]@{
        validation_mode = 'PostPush'
        validation_state = $expectedState
        active_task = 'P7.1-003-R1'
        repair_allowlist_paths = '10'
        primary_commit_paths = '73'
        protected_worktree_rows = '0'
        protected_worktree_profile = 'CLEAN_CHECKOUT_0'
        effective_core_autocrlf = 'true'
        observed_crlf_files = $null
        forced_false_status_rows = $null
        raw_blob_mismatch_files = $null
        rutgers_requests = '0'
        database_mutations = '0'
        package_builds = '0'
        vultr_mutations = '0'
        release_publications = '0'
        production_mutations = '0'
        p7_1_003_r1_gate = 'PASS'
    }
    if ($validatorOutput.Count -ne $expectedValues.Count) {
        Stop-Replay -Code 'VALIDATOR_OUTPUT_LINE_COUNT_MISMATCH'
    }
    $values = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach ($line in $validatorOutput) {
        if ($line -notmatch '^([a-z0-9_]+)=(.*)$') {
            Stop-Replay -Code 'VALIDATOR_OUTPUT_NOT_STRUCTURED'
        }
        $key = [string]$matches[1]
        $value = [string]$matches[2]
        if ($values.ContainsKey($key)) {
            Stop-Replay -Code 'VALIDATOR_OUTPUT_DUPLICATE_KEY'
        }
        $values.Add($key, $value)
    }
    if ($values.Count -ne $expectedValues.Count) {
        Stop-Replay -Code 'VALIDATOR_OUTPUT_KEY_COUNT_MISMATCH'
    }
    foreach ($entry in $expectedValues.GetEnumerator()) {
        if (-not $values.ContainsKey([string]$entry.Key)) {
            Stop-Replay -Code 'VALIDATOR_OUTPUT_KEY_SET_MISMATCH'
        }
        if ($null -ne $entry.Value -and $values[[string]$entry.Key] -cne [string]$entry.Value) {
            Stop-Replay -Code 'VALIDATOR_OUTPUT_VALUE_MISMATCH'
        }
    }

    $numericCounts = [ordered]@{}
    foreach ($countKey in @('observed_crlf_files', 'forced_false_status_rows', 'raw_blob_mismatch_files')) {
        $rawCount = [string]$values[$countKey]
        $parsedCount = 0
        $minimum = if ($countKey -ceq 'forced_false_status_rows') { 0 } else { 1 }
        if ($rawCount -notmatch '^[0-9]+$' -or -not [int]::TryParse($rawCount, [ref]$parsedCount) -or $parsedCount -lt $minimum) {
            Stop-Replay -Code 'VALIDATOR_WINDOWS_PROOF_INCOMPLETE'
        }
        $numericCounts[$countKey] = $parsedCount
    }

    $normalStatus = @(Invoke-GitChecked -Arguments @('status', '--porcelain=v1', '--untracked-files=all', '--no-renames') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_STATUS_FAILED')
    $headAfterExecution = Get-SingleGitLine -Arguments @('rev-parse', 'HEAD') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_HEAD_INVALID'
    $branchAfterExecution = Get-SingleGitLine -Arguments @('branch', '--show-current') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_BRANCH_INVALID'
    $trackingAfterExecution = Get-SingleGitLine -Arguments @('rev-parse', "refs/remotes/origin/$expectedBranch") -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_TRACKING_INVALID'
    $originAfterExecution = Get-SingleGitLine -Arguments @('remote', 'get-url', 'origin') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_ORIGIN_INVALID'
    $effectiveAutocrlf = Get-SingleGitLine -Arguments @('config', '--get', 'core.autocrlf') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_AUTOCRLF_INVALID'
    $reflogAfterExecution = @(Invoke-GitChecked -Arguments @('reflog', 'show', '--format=%H%x09%gs', 'HEAD') -WorkingDirectory $clone -FailureCode 'POST_VALIDATOR_REFLOG_FAILED')
    $remoteAfter = Get-LiveRemoteHead -Remote $remote -Branch $expectedBranch -WorkingDirectory $repo -FailureCode 'REMOTE_AFTER_INVALID'
    if ($headAfterExecution -cne $expectedHead -or $branchAfterExecution -cne $expectedBranch -or $trackingAfterExecution -cne $expectedHead -or $originAfterExecution -cne $expectedRemote -or $effectiveAutocrlf -cne 'true' -or $normalStatus.Count -ne 0 -or $reflogAfterExecution.Count -ne 1 -or $reflogAfterExecution[0] -cne $reflogLine -or $remoteAfter -cne $expectedHead) {
        Stop-Replay -Code 'POST_VALIDATOR_BOUNDARY_MISMATCH'
    }

    $validatorText = ($validatorOutput -join "`n") + "`n"
    $validatorBytes = [Text.UTF8Encoding]::new($false).GetBytes($validatorText)
    $successResult = [ordered]@{
        schemaVersion = 1
        purpose = $Purpose
        state = 'PASS'
        predecessorTask = 'P7.1-003-R1'
        predecessorCommit = $expectedHead
        branch = $expectedBranch
        requiredState = $expectedState
        observedState = [string]$values['validation_state']
        gate = [string]$values['p7_1_003_r1_gate']
        remoteBefore = $remoteBefore
        remoteAfter = $remoteAfter
        freshClone = $true
        cloneReflogEntries = $reflogAfterExecution.Count
        checkoutProfile = [string]$values['protected_worktree_profile']
        effectiveCoreAutocrlf = $effectiveAutocrlf
        normalStatusRows = $normalStatus.Count
        observedCrlfFiles = [int]$numericCounts['observed_crlf_files']
        forcedFalseStatusRows = [int]$numericCounts['forced_false_status_rows']
        rawBlobMismatchFiles = [int]$numericCounts['raw_blob_mismatch_files']
        repairAllowlistPaths = [int]$values['repair_allowlist_paths']
        primaryCommitPaths = [int]$values['primary_commit_paths']
        immutableIdentityCheckedBeforeExecution = $true
        validatorSourceBlobOid = $validatorBlob
        validatorSchemaKeys = $values.Count
        validatorOutputLines = $validatorOutput.Count
        validatorOutputCanonicalization = 'UTF8_NO_BOM_LF_FINAL_LF'
        validatorOutputSha256 = Get-UpperSha256 -Bytes $validatorBytes
        junctionsCreated = $createdJunctions.Count
        cleanupState = 'PENDING'
        negativeSideEffects = [ordered]@{
            rutgersRequests = [int]$values['rutgers_requests']
            databaseMutations = [int]$values['database_mutations']
            packageBuilds = [int]$values['package_builds']
            vultrMutations = [int]$values['vultr_mutations']
            releasePublications = [int]$values['release_publications']
            productionMutations = [int]$values['production_mutations']
        }
        rawCommandOutputPublished = $false
        absoluteLocalPathsPublished = $false
        sensitiveMaterialPublished = $false
        publicationScan = 'PASS'
    }
}
catch {
    $failureCode = Get-ReplayFailureCode -ErrorRecord $_
}

if ($cloneOwned) {
    try {
        $cleanupState = Remove-ReplayClone -ClonePath $clone -CacheRoot $cacheRoot -CreatedJunctions @($createdJunctions)
    }
    catch {
        $cleanupState = 'FAIL'
        if ($null -eq $failureCode) {
            $failureCode = 'CLEANUP_FAILED'
        }
    }
}

if ($null -eq $failureCode -and ($null -eq $successResult -or $cleanupState -cne 'PASS')) {
    $failureCode = 'INCOMPLETE_REPLAY_RESULT'
}

$exitCode = 0
if ($null -eq $failureCode) {
    $successResult.cleanupState = $cleanupState
    $outputObject = $successResult
}
else {
    $exitCode = 1
    $outputObject = [ordered]@{
        schemaVersion = 1
        purpose = $Purpose
        state = 'FAIL'
        gate = 'FAIL'
        failureCode = $failureCode
        cleanupState = $cleanupState
        rawCommandOutputPublished = $false
        absoluteLocalPathsPublished = $false
        sensitiveMaterialPublished = $false
        publicationScan = 'PASS'
    }
}

$json = $outputObject | ConvertTo-Json -Depth 8
if (-not (Test-PublicJsonSafe -Json $json)) {
    $exitCode = 1
    $outputObject = [ordered]@{
        schemaVersion = 1
        purpose = $Purpose
        state = 'FAIL'
        gate = 'FAIL'
        failureCode = 'PUBLICATION_SCAN_FAILED'
        cleanupState = $cleanupState
        rawCommandOutputPublished = $false
        absoluteLocalPathsPublished = $false
        sensitiveMaterialPublished = $false
        publicationScan = 'PASS'
    }
    $json = $outputObject | ConvertTo-Json -Depth 4
    if (-not (Test-PublicJsonSafe -Json $json)) {
        Write-Output '{"schemaVersion":1,"state":"FAIL","gate":"FAIL","failureCode":"PUBLICATION_SCAN_FAILED","rawCommandOutputPublished":false,"absoluteLocalPathsPublished":false,"sensitiveMaterialPublished":false}'
        exit 1
    }
}

Write-Output $json
if ($exitCode -ne 0) {
    exit $exitCode
}
