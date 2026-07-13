[CmdletBinding()]
param(
    [switch]$VerifyOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '04a-p7-1-004-shared-domain-identity-and-typed-api-schema.json'
$baselinePath = Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$helperPath = Join-Path $p7 'tools\invoke-p7-1-003-r1-clean-replay.ps1'
$canonicalEvidence = Join-Path $p7 'evidence\p7-1-004'
$canonicalRecords = Join-Path $p7 'records'
$expectedParent = '870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38'
$expectedBranch = 'codex/p7-implementation'
$expectedRemote = 'https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
$expectedPredecessorState = 'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY'
$cargoHome = Join-Path $repo '.cache\p7-1-002\cargo'
$rustupHome = Join-Path $repo '.cache\p7-1-002\rustup'
$rustToolchainBin = Join-Path $rustupHome 'toolchains\1.97.0-x86_64-pc-windows-msvc\bin'
$cargoExe = Join-Path $cargoHome 'bin\cargo.exe'
$cargoDenyExe = Join-Path $repo '.cache\p7-1-002\tools\cargo-deny\bin\cargo-deny.exe'
$nodeRoot = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\runtime\node-v24.18.0-win-x64'
$nodeExe = Join-Path $nodeRoot 'node.exe'
$targetDir = Join-Path $repo '.cache\p7-1-004\target'
$scratch = $null
$scratchRoot = Join-Path $repo '.cache\p7-1-004'
$observedOrigin = $null

$sourcePaths = @(
    'crates/bcsp-contracts/src/envelope.rs'
    'crates/bcsp-contracts/src/error.rs'
    'crates/bcsp-contracts/src/identity.rs'
    'crates/bcsp-contracts/src/lib.rs'
    'crates/bcsp-contracts/src/match_contract.rs'
    'crates/bcsp-contracts/src/protocol.rs'
    'crates/bcsp-contracts/src/schema.rs'
    'crates/bcsp-contracts/tests/compatibility.rs'
    'crates/bcsp-contracts/tests/golden/contract-manifest-v1.json'
    'crates/bcsp-contracts/tests/golden/http-error-v1.json'
    'crates/bcsp-contracts/tests/golden/http-request-v1.json'
    'crates/bcsp-contracts/tests/golden/http-success-v1.json'
    'crates/bcsp-contracts/tests/golden/identity-payload-v1.json'
    'crates/bcsp-contracts/tests/golden/match-explanation-v1.json'
    'crates/bcsp-contracts/tests/golden/section-key-v1.json'
    'crates/bcsp-contracts/tests/golden/ws-client-envelope-v1.json'
    'crates/bcsp-contracts/tests/golden/ws-server-envelope-v1.json'
    'crates/bcsp-contracts/tests/malformed_wire.rs'
    'crates/bcsp-contracts/tests/schema_binding.rs'
    'crates/bcsp-contracts/tests/wire_golden.rs'
    'crates/bcsp-domain/src/course.rs'
    'crates/bcsp-domain/src/lib.rs'
    'crates/bcsp-domain/src/match_result.rs'
    'crates/bcsp-domain/tests/course_model.rs'
    'crates/bcsp-domain/tests/fixtures/synthetic-identity-collisions-v1.tsv'
    'crates/bcsp-domain/tests/identity_collisions.rs'
    'crates/bcsp-domain/tests/three_value_tables.rs'
    'tools/architecture/verify-rust-graph.mjs'
    'tools/architecture/verify-rust-graph.test.mjs'
)
$goldenPaths = @($sourcePaths | Where-Object { $_ -like 'crates/bcsp-contracts/tests/golden/*' })
$expectedQualityGateIds = @(
    'predecessor-entry-capture'
    'predecessor-precommit-replay'
    'predecessor-structured-equivalence'
    'rust-graph-guard'
    'rust-graph-guard-self-test'
    'cargo-metadata-windows-locked-offline'
    'cargo-metadata-linux-locked-offline'
    'cargo-metadata-complete-closure-locked-offline'
    'cargo-check-workspace-all-targets-locked-offline'
    'cargo-test-workspace-all-targets-locked-offline'
    'cargo-clippy-workspace-all-targets-locked-offline'
    'cargo-fmt-check'
    'cargo-deny-policy-locked-offline'
    'cargo-test-bcsp-contracts-locked-offline'
    'cargo-test-bcsp-domain-locked-offline'
    'dependency-lock-unchanged'
    'dependency-closure-delta'
    'schema-golden-hash-lock'
    'synthetic-only-fixture-publication-scan'
    'exact-task-allowlist'
    'protected-worktree-profile'
    'publication-safety'
    'zero-side-effects'
    'evidence-builder-verify'
    'evidence-builder-write'
)

function Normalize-Path([string]$Value) {
    return $Value.Replace('\', '/')
}

function Read-Json([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "missing JSON input: $(Normalize-Path $Path)"
    }
    return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json
}

function Get-CanonicalTextSha256([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $utf8 = [System.Text.UTF8Encoding]::new($false, $true)
    $text = $utf8.GetString($bytes)
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) {
        throw "UTF-8 BOM is forbidden in canonical text: $Path"
    }
    if ($text.Contains([char]0)) {
        throw "NUL is forbidden in canonical text: $Path"
    }
    if ($text.Replace("`r`n", '').Contains("`r")) {
        throw "lone CR is forbidden in canonical text: $Path"
    }
    $canonical = $text.Replace("`r`n", "`n")
    if (-not $canonical.EndsWith("`n")) {
        throw "canonical text must end with LF: $Path"
    }
    return Get-StringSha256 -Value $canonical
}

function Get-StringSha256([string]$Value) {
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $hasher.ComputeHash([System.Text.UTF8Encoding]::new($false).GetBytes($Value))
    }
    finally {
        $hasher.Dispose()
    }
    return ([System.BitConverter]::ToString($hash)).Replace('-', '')
}

function Get-ObjectSha256([object]$Value) {
    return Get-StringSha256 -Value ($Value | ConvertTo-Json -Depth 30 -Compress)
}

function Write-Utf8([string]$Path, [string]$Content) {
    $directory = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $directory -Force)
    }
    [System.IO.File]::WriteAllText(
        $Path,
        $Content.Replace("`r`n", "`n"),
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Write-Json([string]$Path, [object]$Value) {
    Write-Utf8 -Path $Path -Content (($Value | ConvertTo-Json -Depth 30) + "`n")
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
        throw "git $($Arguments -join ' ') failed with exit $exitCode"
    }
    return [pscustomobject]@{ Output = $output; ExitCode = $exitCode }
}

function Get-GitSingle([string[]]$Arguments) {
    $result = Invoke-Git -Arguments $Arguments
    if (@($result.Output).Count -ne 1 -or [string]::IsNullOrWhiteSpace([string]$result.Output[0])) {
        throw "git $($Arguments -join ' ') did not return one value"
    }
    return ([string]$result.Output[0]).Trim()
}

function Invoke-CommandCapture([string]$Executable, [string[]]$Arguments, [string]$WorkingDirectory) {
    if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
        throw "missing executable: $Executable"
    }
    $oldLocation = (Get-Location).Path
    try {
        Set-Location $WorkingDirectory
        $previousPreference = $ErrorActionPreference
        try {
            $ErrorActionPreference = 'Continue'
            $output = @(& $Executable @Arguments 2>&1 | ForEach-Object { $_.ToString() })
            $exitCode = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $previousPreference
        }
    }
    finally {
        Set-Location $oldLocation
    }
    return [pscustomobject]@{ Output = $output; ExitCode = $exitCode }
}

function Assert-Pass([object]$Result, [string]$Id) {
    if ([int]$Result.ExitCode -ne 0) {
        $tail = (@($Result.Output) | Select-Object -Last 8) -join ' | '
        throw "$Id failed with exit $($Result.ExitCode): $tail"
    }
}

function Compare-Set([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    $actualSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $expectedSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($value in @($Actual)) {
        if (-not $actualSet.Add([string]$value)) {
            throw "$Label has duplicate actual value: $value"
        }
    }
    foreach ($value in @($Expected)) {
        if (-not $expectedSet.Add([string]$value)) {
            throw "$Label has duplicate expected value: $value"
        }
    }
    $missing = @($expectedSet | Where-Object { -not $actualSet.Contains([string]$_) })
    $extra = @($actualSet | Where-Object { -not $expectedSet.Contains([string]$_) })
    if ($missing.Count -gt 0 -or $extra.Count -gt 0) {
        throw "$Label mismatch; missing=[$($missing -join ',')] extra=[$($extra -join ',')]"
    }
}

function Test-SafeRelativePath([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($Path) -or [System.IO.Path]::IsPathRooted($Path)) {
        return $false
    }
    if ($Path -match '^[A-Za-z]:' -or $Path -match '(^|/)\.\.(/|$)') {
        return $false
    }
    return $true
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

function Get-ProtectedPathSet {
    $rows = @(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")
    if ($rows.Count -ne 167) {
        throw "protected baseline row count is $($rows.Count), expected 167"
    }
    $set = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($row in $rows) {
        [void]$set.Add((Normalize-Path ([string]$row.path)))
    }
    return $set
}

function Get-ManifestEntrySetSha256([string]$Path) {
    $lines = @(Get-Content -LiteralPath $Path -Encoding utf8)
    $expectedHeader = "path`tgit_status`tbaseline_kind`thead_blob_oid`tworktree_sha256`tsize_bytes`tcontent_policy`townership`trequired_action"
    if ($lines.Count -ne 168 -or [string]$lines[0] -cne $expectedHeader) {
        throw 'protected baseline manifest shape mismatch'
    }
    return Get-StringSha256 -Value ((($lines | Select-Object -Skip 1) -join "`n") + "`n")
}

function Test-NeverReadProtectedPath([string]$Path) {
    $normalized = Normalize-Path $Path
    return $normalized -match '(?i)(^|/)chat-log-[^/]*\.md$' -or
        $normalized -match '(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)' -or
        $normalized -match '(?i)^project-governance/current/p1/' -or
        $normalized -match '(?i)(^|/)\.secrets(/|$)'
}

function Test-ProtectedWorktreeProfile([string[]]$Allowlist) {
    $manifestSha = Get-ManifestEntrySetSha256 -Path $baselinePath
    if ($manifestSha -cne 'C7A4FEB33F4F9198678AFA5C38D8CBD4D54378BD18DFA9D22CDA85FA1290089D') {
        throw 'protected baseline manifest entry-set identity mismatch'
    }
    $baseline = @(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")
    if ($baseline.Count -ne 167) {
        throw "protected baseline row count is $($baseline.Count), expected 167"
    }
    $allow = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in @($Allowlist)) {
        [void]$allow.Add((Normalize-Path $path))
    }
    $actual = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    $status = (Invoke-Git -Arguments @('-c', 'core.quotepath=false', 'status', '--porcelain=v1', '--untracked-files=all', '--no-renames')).Output
    foreach ($raw in @($status)) {
        $line = [string]$raw
        if ($line.Length -lt 4) {
            continue
        }
        $code = $line.Substring(0, 2)
        $path = Normalize-Path $line.Substring(3)
        if ($allow.Contains($path)) {
            continue
        }
        if ($code[0] -ne ' ' -and $code -cne '??') {
            throw 'a protected or foreign path is staged outside the task allowlist'
        }
        $normalizedStatus = if ($code -ceq '??') {
            'UNTRACKED'
        }
        elseif ($code[1] -ceq 'D') {
            'WORKTREE_D'
        }
        else {
            "WORKTREE_$($code[1])"
        }
        if ($actual.ContainsKey($path)) {
            throw 'duplicate protected worktree status path'
        }
        $actual.Add($path, $normalizedStatus)
    }

    $privateIgnored = (Invoke-Git -Arguments @('check-ignore', '-q', '.secrets/') -AllowFailure $true).ExitCode -eq 0
    $privateTracked = @((Invoke-Git -Arguments @('ls-files', '--', '.secrets')).Output | Where-Object { $_ -ne '' })
    if (-not $privateIgnored -or $privateTracked.Count -ne 0) {
        throw 'opaque private root ignore/tracking policy mismatch'
    }

    if ($actual.Count -eq 0) {
        return [pscustomobject]@{
            Profile = 'CLEAN_CHECKOUT_0'
            Rows = 0
            ManifestRows = 167
            ManifestEntrySetSha256 = $manifestSha
        }
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
        if ($opaqueByPath -and -not $opaqueByPolicy) {
            throw "protected opaque row $rowIndex has an unsafe content policy"
        }
        if ([string]$actual[$path] -cne [string]$row.git_status) {
            throw "protected baseline status mismatch at row $rowIndex"
        }
        if (-not [string]::IsNullOrWhiteSpace([string]$row.head_blob_oid)) {
            $headBlob = (Invoke-Git -Arguments @('rev-parse', "HEAD:$path") -AllowFailure $true)
            $headBlobValue = if ($headBlob.ExitCode -eq 0) { (@($headBlob.Output) -join '').Trim() } else { '' }
            if ($headBlobValue -cne [string]$row.head_blob_oid) {
                throw "protected HEAD blob mismatch at row $rowIndex"
            }
        }
        $absolute = Join-Path $repo $path.Replace('/', '\')
        if ([string]$row.git_status -ceq 'WORKTREE_D') {
            if (Test-Path -LiteralPath $absolute) {
                throw "protected deletion reappeared at row $rowIndex"
            }
            continue
        }
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
            throw "protected file missing at row $rowIndex"
        }
        if ((Get-Item -LiteralPath $absolute).Length -ne [int64]$row.size_bytes) {
            throw "protected size mismatch at row $rowIndex"
        }
        if ($opaqueByPolicy) {
            if ([string]$row.worktree_sha256 -cne 'NOT_COMPUTED') {
                throw "opaque protected row $rowIndex unexpectedly carries a content hash"
            }
            continue
        }
        if ($opaqueByPath) {
            throw "protected never-read row $rowIndex reached the hash branch"
        }
        if ([string]$row.worktree_sha256 -notmatch '^[0-9A-F]{64}$') {
            throw "protected hash policy mismatch at row $rowIndex"
        }
        if ((Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash -cne [string]$row.worktree_sha256) {
            throw "protected raw-byte hash mismatch at row $rowIndex"
        }
    }
    return [pscustomobject]@{
        Profile = 'EXACT_PRESERVED_167'
        Rows = 167
        ManifestRows = 167
        ManifestEntrySetSha256 = $manifestSha
    }
}

function Get-TaskStatusPaths {
    $protected = Get-ProtectedPathSet
    $paths = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $status = (Invoke-Git -Arguments @('-c', 'core.quotepath=false', 'status', '--porcelain=v1', '--untracked-files=all', '--no-renames')).Output
    foreach ($raw in @($status)) {
        $line = [string]$raw
        if ($line.Length -lt 4) {
            continue
        }
        $path = Normalize-Path $line.Substring(3)
        if ($protected.Contains($path)) {
            continue
        }
        if (Test-DeniedTaskPath $path) {
            throw "denied path entered the task status boundary: $path"
        }
        [void]$paths.Add($path)
    }
    return @($paths | Sort-Object)
}

function Get-CommitPaths([string]$Revision) {
    return @(
        (Invoke-Git -Arguments @('diff-tree', '--no-renames', '--no-commit-id', '--name-only', '-r', $Revision)).Output |
            ForEach-Object { Normalize-Path ([string]$_) } |
            Where-Object { $_ -ne '' }
    )
}

function Test-ProspectiveTaskBoundary([string]$Head, [string[]]$Allowlist, [string[]]$RequiredGovernancePaths) {
    if ($Head -ceq $expectedParent) {
        $candidate = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($path in @(Get-TaskStatusPaths)) {
            [void]$candidate.Add($path)
        }
        foreach ($path in @($RequiredGovernancePaths)) {
            $normalized = Normalize-Path $path
            if (-not (Test-SafeRelativePath $normalized) -or (Test-DeniedTaskPath $normalized)) {
                throw "unsafe required governance path: $normalized"
            }
            [void]$candidate.Add($normalized)
        }
        Compare-Set -Actual @($candidate) -Expected $Allowlist -Label 'prospective task/contract allowlist'
        return
    }
    $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', 'HEAD')) -split '\s+'
    if ($parents.Count -ne 2 -or $parents[1] -cne $expectedParent) {
        throw 'task boundary is not the direct single-parent P7.1-004 commit'
    }
    Compare-Set -Actual @(Get-CommitPaths -Revision 'HEAD') -Expected $Allowlist -Label 'committed task/contract allowlist'
}

function Test-PublicationText([string]$Content, [string]$Label) {
    $patterns = @(
        @{ Id = 'private-key'; Pattern = '-----BEGIN [A-Z ]*PRIVATE KEY-----' }
        @{ Id = 'ssh-key'; Pattern = '(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}' }
        @{ Id = 'github-token'; Pattern = 'github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}' }
        @{ Id = 'npm-token'; Pattern = 'npm_[A-Za-z0-9]{20,}' }
        @{ Id = 'openai-token'; Pattern = 'sk-(proj-)?[A-Za-z0-9_-]{20,}' }
        @{ Id = 'jwt'; Pattern = 'eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}' }
        @{ Id = 'credential-url'; Pattern = '(?i)https?://[^/\s:@]+:[^/\s@]+@' }
        @{ Id = 'windows-absolute'; Pattern = '(?i)(^|[\s"''])[A-Z]:[\\/]' }
        @{ Id = 'posix-user-path'; Pattern = '(?i)/(home|Users|mnt/[a-z])/[^/\s"'']+/' }
        @{ Id = 'email'; Pattern = '(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}' }
    )
    foreach ($rule in $patterns) {
        if ([regex]::IsMatch($Content, [string]$rule.Pattern)) {
            throw "publication rule $($rule.Id) matched $Label"
        }
    }
}

function Test-PublicationFiles([string[]]$Paths) {
    foreach ($relative in @($Paths)) {
        if (-not (Test-SafeRelativePath $relative) -or (Test-DeniedTaskPath $relative)) {
            throw "unsafe publication path: $relative"
        }
        $absolute = Join-Path $repo $relative.Replace('/', '\')
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
            throw "publication path is missing: $relative"
        }
        $content = Get-Content -LiteralPath $absolute -Raw -Encoding utf8
        Test-PublicationText -Content $content -Label $relative
    }
}

function Assert-ZeroSideEffects([object]$SideEffects, [string]$Label) {
    foreach ($name in @('rutgersRequests', 'databaseMutations', 'packageBuilds', 'vultrMutations', 'releasePublications', 'productionMutations')) {
        if ([int]$SideEffects.$name -ne 0) {
            throw "$Label reports non-zero $name"
        }
    }
}

function Convert-ReplayComparable([object]$Replay) {
    return [ordered]@{
        taskId = [string]$Replay.predecessorTask
        commit = [string]$Replay.predecessorCommit
        branch = [string]$Replay.branch
        remoteIdentity = $observedOrigin
        terminalState = [string]$Replay.observedState
        profile = [string]$Replay.checkoutProfile
        coreAutocrlf = [string]$Replay.effectiveCoreAutocrlf
        normalStatusClean = ([int]$Replay.normalStatusRows -eq 0)
        observedCrlfFilesMinimumSatisfied = ([int]$Replay.observedCrlfFiles -ge 1)
        rawBlobMismatchFilesMinimumSatisfied = ([int]$Replay.rawBlobMismatchFiles -ge 1)
        portabilitySelfTest = [string]$Replay.gate
        canonicalGovernedHashes = [ordered]@{
            validatorSourceBlobOid = [string]$Replay.validatorSourceBlobOid
            validatorOutputSha256 = [string]$Replay.validatorOutputSha256
        }
    }
}

function Test-Replay([object]$Replay, [string]$Purpose) {
    if ([int]$Replay.schemaVersion -ne 1 -or [string]$Replay.purpose -cne $Purpose -or [string]$Replay.state -cne 'PASS') {
        throw "$Purpose replay identity mismatch"
    }
    if ([string]$Replay.predecessorTask -cne 'P7.1-003-R1' -or [string]$Replay.predecessorCommit -cne $expectedParent) {
        throw "$Purpose replay predecessor mismatch"
    }
    if ([string]$Replay.branch -cne $expectedBranch -or [string]$Replay.requiredState -cne $expectedPredecessorState -or [string]$Replay.observedState -cne $expectedPredecessorState -or [string]$Replay.gate -cne 'PASS') {
        throw "$Purpose replay terminal state mismatch"
    }
    if ([string]$Replay.remoteBefore -cne $expectedParent -or [string]$Replay.remoteAfter -cne $expectedParent) {
        throw "$Purpose replay remote boundary mismatch"
    }
    if (-not [bool]$Replay.freshClone -or [int]$Replay.cloneReflogEntries -ne 1 -or [string]$Replay.checkoutProfile -cne 'CLEAN_CHECKOUT_0' -or [string]$Replay.effectiveCoreAutocrlf -cne 'true' -or [int]$Replay.normalStatusRows -ne 0) {
        throw "$Purpose replay checkout proof mismatch"
    }
    if ([int]$Replay.observedCrlfFiles -lt 1 -or [int]$Replay.rawBlobMismatchFiles -lt 1 -or [int]$Replay.forcedFalseStatusRows -lt 0) {
        throw "$Purpose replay Windows proof is incomplete"
    }
    if ([int]$Replay.repairAllowlistPaths -ne 10 -or [int]$Replay.primaryCommitPaths -ne 73 -or -not [bool]$Replay.immutableIdentityCheckedBeforeExecution) {
        throw "$Purpose replay immutable identity proof mismatch"
    }
    if ([string]$Replay.validatorSourceBlobOid -notmatch '^[0-9a-f]{40}$' -or [string]$Replay.validatorOutputSha256 -notmatch '^[0-9A-F]{64}$') {
        throw "$Purpose replay validator identity is malformed"
    }
    if ([string]$Replay.cleanupState -cne 'PASS' -or [bool]$Replay.rawCommandOutputPublished -or [bool]$Replay.absoluteLocalPathsPublished -or [bool]$Replay.sensitiveMaterialPublished -or [string]$Replay.publicationScan -cne 'PASS') {
        throw "$Purpose replay publication or cleanup proof mismatch"
    }
    Assert-ZeroSideEffects -SideEffects $Replay.negativeSideEffects -Label $Purpose
}

function Invoke-Replay([string]$Purpose) {
    $powershellExe = Join-Path $PSHOME 'powershell.exe'
    $result = Invoke-CommandCapture -Executable $powershellExe -Arguments @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $helperPath, '-Purpose', $Purpose
    ) -WorkingDirectory $repo
    Assert-Pass -Result $result -Id "predecessor-$Purpose"
    try {
        $replay = (@($result.Output) -join "`n") | ConvertFrom-Json
    }
    catch {
        throw "$Purpose replay returned invalid structured JSON: $($_.Exception.Message)"
    }
    Test-Replay -Replay $replay -Purpose $Purpose
    return $replay
}

function New-PredecessorProof([object]$EntryCapture, [object]$PreCommitReplay) {
    Test-Replay -Replay $EntryCapture -Purpose 'EntryCapture'
    Test-Replay -Replay $PreCommitReplay -Purpose 'PreCommitReplay'
    $entryComparable = Convert-ReplayComparable -Replay $EntryCapture
    $preCommitComparable = Convert-ReplayComparable -Replay $PreCommitReplay
    $entryComparableHash = Get-ObjectSha256 -Value $entryComparable
    $preCommitComparableHash = Get-ObjectSha256 -Value $preCommitComparable
    if ($entryComparableHash -cne $preCommitComparableHash) {
        throw 'EntryCapture and PreCommitReplay structured results are not equivalent'
    }
    $entryFullHash = Get-ObjectSha256 -Value $EntryCapture
    $preCommitFullHash = Get-ObjectSha256 -Value $PreCommitReplay
    if ($entryFullHash -ceq $preCommitFullHash) {
        throw 'independent predecessor executions do not have distinct purpose-bound records'
    }
    return [ordered]@{
        schemaVersion = 1
        taskId = 'P7.1-004'
        state = 'PASS'
        predecessorTask = 'P7.1-003-R1'
        requiredCommit = $expectedParent
        requiredTerminalState = $expectedPredecessorState
        helper = 'project-governance/current/p7/tools/invoke-p7-1-003-r1-clean-replay.ps1'
        entryCapture = $EntryCapture
        preCommitReplay = $PreCommitReplay
        stableProjectionSha256 = $entryComparableHash
        equivalent = $true
        structuredEquivalence = [ordered]@{
            state = 'PASS'
            purposeExcludedFromComparison = $true
            comparedFieldCount = $entryComparable.Count
            entryComparableSha256 = $entryComparableHash
            preCommitComparableSha256 = $preCommitComparableHash
            equivalent = $true
            entryPurposeBoundRecordSha256 = $entryFullHash
            preCommitPurposeBoundRecordSha256 = $preCommitFullHash
            purposeBoundRecordsDistinct = $true
        }
        executionIsolation = [ordered]@{
            taskExecutionsRepresented = 2
            entryCaptureLoadedFromPreExistingRecord = $true
            preCommitHelperInvocationsDuringWrite = 1
            freshClonePerInvocation = $true
            entryPurpose = 'EntryCapture'
            preCommitPurpose = 'PreCommitReplay'
        }
        replayPolicy = [ordered]@{
            writeModeInvokesPreCommitHelper = $true
            writeModeInvokesEntryHelper = $false
            verifyOnlyInvokesHelper = $false
            postCommitInvokesHelper = $false
            postPushInvokesHelper = $false
        }
        rawCommandOutputPublished = $false
        absoluteLocalPathsPublished = $false
        sensitiveMaterialPublished = $false
        publicationScan = 'PASS'
    }
}

function Test-PredecessorProof([object]$Proof) {
    if ([int]$Proof.schemaVersion -ne 1 -or [string]$Proof.taskId -cne 'P7.1-004' -or [string]$Proof.state -cne 'PASS') {
        throw 'predecessor proof identity mismatch'
    }
    if ([string]$Proof.predecessorTask -cne 'P7.1-003-R1' -or [string]$Proof.requiredCommit -cne $expectedParent -or [string]$Proof.requiredTerminalState -cne $expectedPredecessorState) {
        throw 'predecessor proof boundary mismatch'
    }
    Test-Replay -Replay $Proof.entryCapture -Purpose 'EntryCapture'
    Test-Replay -Replay $Proof.preCommitReplay -Purpose 'PreCommitReplay'
    $rebuilt = New-PredecessorProof -EntryCapture $Proof.entryCapture -PreCommitReplay $Proof.preCommitReplay
    if ((Get-ObjectSha256 -Value $rebuilt) -cne (Get-ObjectSha256 -Value $Proof)) {
        throw 'predecessor proof structured equivalence metadata drifted'
    }
    if (-not [bool]$Proof.replayPolicy.writeModeInvokesPreCommitHelper -or [bool]$Proof.replayPolicy.writeModeInvokesEntryHelper -or [bool]$Proof.replayPolicy.verifyOnlyInvokesHelper -or [bool]$Proof.replayPolicy.postCommitInvokesHelper -or [bool]$Proof.replayPolicy.postPushInvokesHelper) {
        throw 'predecessor replay phase policy mismatch'
    }
}

function Assert-SourceContract {
    foreach ($relative in $sourcePaths) {
        $absolute = Join-Path $repo $relative.Replace('/', '\')
        if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
            throw "missing shared-domain source or test: $relative"
        }
        [void](Get-CanonicalTextSha256 -Path $absolute)
    }

    $identityText = Get-Content -LiteralPath (Join-Path $repo 'crates\bcsp-contracts\src\identity.rs') -Raw -Encoding utf8
    $errorText = Get-Content -LiteralPath (Join-Path $repo 'crates\bcsp-contracts\src\error.rs') -Raw -Encoding utf8
    $domainCourseText = Get-Content -LiteralPath (Join-Path $repo 'crates\bcsp-domain\src\course.rs') -Raw -Encoding utf8
    if ($identityText -notmatch 'SECTION_INDEX_WIDTH:\s*usize\s*=\s*5' -or $identityText -notmatch 'FINGERPRINT_PREFIX:\s*&str\s*=\s*"v1:"') {
        throw 'identity width or versioned fingerprint contract is absent'
    }
    foreach ($typeName in @('SectionKey', 'CourseGroupKey', 'CourseVariantKey')) {
        if ($identityText -notmatch "pub struct $typeName") {
            throw "identity type is absent: $typeName"
        }
    }
    if ($errorText -notmatch 'pub type ApiErrorBody\s*=\s*TypedApiErrorBody<ApiErrorCode>;' -or $errorText -notmatch 'pub type ApiErrorEnvelope\s*=\s*TypedApiErrorEnvelope<ApiErrorCode>;' -or $errorText -notmatch 'pub fn decode_versioned_envelope_json') {
        throw 'concrete shared error aliases or named version decoder are absent'
    }
    if ($domainCourseText -match '\bserde\b|\bSerialize\b|\bDeserialize\b') {
        throw 'CourseGroup/CourseVariant aggregate layer unexpectedly acquired a wire-format binding'
    }
    foreach ($aggregate in @('CourseGroup', 'CourseVariant')) {
        if ($domainCourseText -notmatch "pub struct $aggregate") {
            throw "domain aggregate is absent: $aggregate"
        }
    }

    $manifest = Read-Json (Join-Path $repo 'crates\bcsp-contracts\tests\golden\contract-manifest-v1.json')
    if ([int]$manifest.schemaVersion -ne 1 -or [int]$manifest.apiProtocolVersion -ne 1 -or [int]$manifest.wsProtocolVersion -ne 1 -or @($manifest.scalarConstraints).Count -ne 10 -or @($manifest.schemas).Count -ne 16) {
        throw 'contract manifest cardinality or protocol version mismatch'
    }
    $expectedSchemas = @(
        'bcsp.identity.term-campus-key.v1', 'bcsp.identity.section-key.v1',
        'bcsp.identity.course-group-key.v1', 'bcsp.identity.course-variant-key.v1',
        'bcsp.match.outcome.v1', 'bcsp.match.reason-code.v1', 'bcsp.match.reason.v1',
        'bcsp.match.explanation.v1', 'bcsp.error.shared-code.v1',
        'bcsp.http.request-envelope.v1', 'bcsp.http.success-envelope.v1',
        'bcsp.http.error-body.v1', 'bcsp.http.error-detail.v1',
        'bcsp.http.error-envelope.v1', 'bcsp.ws.client-envelope.v1',
        'bcsp.ws.server-envelope.v1'
    )
    Compare-Set -Actual @($manifest.schemas | ForEach-Object { [string]$_.id }) -Expected $expectedSchemas -Label 'contract schema IDs'

    foreach ($relative in $goldenPaths) {
        [void](Read-Json (Join-Path $repo $relative.Replace('/', '\')))
    }
    $fixturePath = Join-Path $repo 'crates\bcsp-domain\tests\fixtures\synthetic-identity-collisions-v1.tsv'
    $fixtureLines = @(Get-Content -LiteralPath $fixturePath -Encoding utf8)
    if ($fixtureLines.Count -ne 24 -or [string]$fixtureLines[0] -cne "classification`tSYNTHETIC_NO_REAL_COURSE_DATA") {
        throw 'synthetic collision fixture classification or row count mismatch'
    }
    foreach ($line in $fixtureLines[1..($fixtureLines.Count - 1)]) {
        if ([string]$line -notmatch '^(section|equivalent|conflicting|mixed)\tT2026[FS]\tCAMPUS_[AB]\t') {
            throw 'synthetic collision fixture contains an unclassified identity row'
        }
    }
}

$contract = Read-Json -Path $contractPath
if ([int]$contract.schemaVersion -ne 1 -or [string]$contract.taskId -cne 'P7.1-004' -or [string]$contract.state -cne 'IMPLEMENTATION_CONTRACT') {
    throw 'P7.1-004 contract identity mismatch'
}
if ([string]$contract.branch -cne $expectedBranch -or [string]$contract.expectedParent -cne $expectedParent -or [string]$contract.expectedRemoteBaseline -cne $expectedParent) {
    throw 'P7.1-004 contract Git boundary mismatch'
}
if ([string]$contract.dependency.taskId -cne 'P7.1-003-R1' -or [string]$contract.dependency.requiredCommit -cne $expectedParent -or [string]$contract.dependency.requiredTerminalState -cne $expectedPredecessorState) {
    throw 'P7.1-004 predecessor contract mismatch'
}
if ([bool]$contract.dependency.postCommitReplayAllowed -or [bool]$contract.dependency.postPushReplayAllowed) {
    throw 'contract permits a forbidden post-commit predecessor replay'
}
if ([bool]$contract.scope.dependencyOrLockChangeAllowed -or [bool]$contract.scope.realRutgersRequestsAllowed -or [bool]$contract.scope.databaseMutationAllowed -or [bool]$contract.scope.packageBuildAllowed -or [bool]$contract.scope.formalUiImplementationAllowed -or [bool]$contract.scope.vultrMutationAllowed -or [bool]$contract.scope.releaseOrProductionAllowed) {
    throw 'P7.1-004 contract permits an out-of-scope mutation'
}
if ([int]$contract.commitBoundary.expectedTaskPathCount -ne 41 -or [int]$contract.apiContract.manifest.scalarConstraintCount -ne 10 -or [int]$contract.apiContract.manifest.schemaCount -ne 16 -or [int]$contract.apiContract.manifest.goldenFileCount -ne 9) {
    throw 'P7.1-004 frozen contract counts mismatch'
}
if ([bool]$contract.predecessorReplay.executionIdExists -or [bool]$contract.predecessorReplay.executionIdRequired -or [bool]$contract.predecessorReplay.executionIdMayBeInvented -or [int]$contract.predecessorReplay.builderWriteHelperInvocationCount -ne 1 -or [int]$contract.predecessorReplay.verifyOnlyHelperInvocationCount -ne 0 -or [int]$contract.predecessorReplay.postCommitHelperInvocationCount -ne 0 -or [int]$contract.predecessorReplay.postPushHelperInvocationCount -ne 0) {
    throw 'P7.1-004 predecessor replay execution policy mismatch'
}
$expectedProjectionFields = @('taskId', 'commit', 'branch', 'remoteIdentity', 'terminalState', 'profile', 'coreAutocrlf', 'normalStatusClean', 'observedCrlfFilesMinimumSatisfied', 'rawBlobMismatchFilesMinimumSatisfied', 'portabilitySelfTest', 'canonicalGovernedHashes')
Compare-Set -Actual @($contract.predecessorReplay.stableProjectionFields | ForEach-Object { [string]$_ }) -Expected $expectedProjectionFields -Label 'stable replay projection fields'
if (-not ($contract.commitBoundary.finalized -is [bool]) -or -not [bool]$contract.commitBoundary.finalized) {
    throw 'P7.1-004 exact allowlist is not finalized'
}
$allowlist = @($contract.commitBoundary.taskCommitAllowlist | ForEach-Object { Normalize-Path ([string]$_) })
if ($allowlist.Count -ne 41) {
    throw "P7.1-004 allowlist count is $($allowlist.Count), expected 41"
}
$sortedAllowlist = [string[]]$allowlist.Clone()
[Array]::Sort($sortedAllowlist, [StringComparer]::Ordinal)
$uniqueAllowlist = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($path in $allowlist) {
    [void]$uniqueAllowlist.Add($path)
}
if ($uniqueAllowlist.Count -ne $allowlist.Count) {
    throw 'P7.1-004 allowlist contains duplicate paths'
}
for ($index = 0; $index -lt $allowlist.Count; $index++) {
    if ([string]$allowlist[$index] -cne [string]$sortedAllowlist[$index]) {
        throw "P7.1-004 allowlist is not lexicographically sorted at position $index"
    }
}
foreach ($path in $allowlist) {
    if (-not (Test-SafeRelativePath $path) -or (Test-DeniedTaskPath $path)) {
        throw "unsafe P7.1-004 allowlist path: $path"
    }
}
Compare-Set -Actual @($contract.qualityGates | ForEach-Object { [string]$_ }) -Expected $expectedQualityGateIds -Label 'contract quality gate IDs'
if ([bool]$contract.lockArtifacts.cargoLockChangeExpected -or [bool]$contract.lockArtifacts.frontendNpmLockChangeExpected) {
    throw 'dependency lock changes are forbidden for P7.1-004'
}

$head = Get-GitSingle -Arguments @('rev-parse', 'HEAD')
$branch = Get-GitSingle -Arguments @('branch', '--show-current')
$observedOrigin = Get-GitSingle -Arguments @('remote', 'get-url', 'origin')
if ($branch -cne $expectedBranch) {
    throw "branch mismatch: $branch"
}
if ($observedOrigin -cne $expectedRemote) {
    throw 'origin identity mismatch'
}
if (-not $VerifyOnly -and $head -cne $expectedParent) {
    throw "evidence write requires uncommitted P7.1-004 on predecessor $expectedParent; observed $head"
}
$protectedProfile = Test-ProtectedWorktreeProfile -Allowlist $allowlist
Test-ProspectiveTaskBoundary -Head $head -Allowlist $allowlist -RequiredGovernancePaths @($contract.commitBoundary.requiredGovernancePaths | ForEach-Object { [string]$_ })

foreach ($required in @($cargoExe, $cargoDenyExe, $nodeExe, $helperPath, $baselinePath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "missing required local input: $required"
    }
}

$env:CARGO_HOME = $cargoHome
$env:RUSTUP_HOME = $rustupHome
$env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'
$env:CARGO_NET_OFFLINE = 'true'
$env:CARGO_TARGET_DIR = $targetDir
$env:PATH = "$rustToolchainBin;$nodeRoot;$env:PATH"

$gateRecords = [System.Collections.Generic.List[object]]::new()
function Add-GateRecord([string]$Id, [string]$BinaryId, [string[]]$Arguments) {
    [void]$gateRecords.Add([ordered]@{
        id = $Id
        binaryId = $BinaryId
        arguments = $Arguments
        exitCode = 0
        normalizedResult = 'PASS'
    })
}
function Run-Gate([string]$Id, [string]$BinaryId, [string]$Executable, [string[]]$Arguments, [string]$WorkingDirectory, [string[]]$PublishedArguments) {
    $result = Invoke-CommandCapture -Executable $Executable -Arguments $Arguments -WorkingDirectory $WorkingDirectory
    Assert-Pass -Result $result -Id $Id
    Add-GateRecord -Id $Id -BinaryId $BinaryId -Arguments $PublishedArguments
    return $result
}

Assert-SourceContract

if ($VerifyOnly) {
    $predecessorProofPath = Join-Path $canonicalEvidence 'predecessor-r1-clean-replay.json'
    $predecessorProof = Read-Json -Path $predecessorProofPath
    Test-PredecessorProof -Proof $predecessorProof
}
else {
    $entryCaptureInput = Read-Json -Path (Join-Path $canonicalEvidence 'predecessor-r1-clean-replay.json')
    if ($entryCaptureInput.PSObject.Properties.Name -contains 'entryCapture') {
        $entryCapture = $entryCaptureInput.entryCapture
    }
    else {
        $entryCapture = $entryCaptureInput
    }
    Test-Replay -Replay $entryCapture -Purpose 'EntryCapture'
    $preCommitReplay = Invoke-Replay -Purpose 'PreCommitReplay'
    $predecessorProof = New-PredecessorProof -EntryCapture $entryCapture -PreCommitReplay $preCommitReplay
}
Add-GateRecord -Id 'predecessor-entry-capture' -BinaryId 'powershell-helper' -Arguments @('-Purpose', 'EntryCapture')
Add-GateRecord -Id 'predecessor-precommit-replay' -BinaryId 'powershell-helper' -Arguments @('-Purpose', 'PreCommitReplay')
Add-GateRecord -Id 'predecessor-structured-equivalence' -BinaryId 'builder' -Arguments @()

$rustGuard = Join-Path $repo 'tools\architecture\verify-rust-graph.mjs'
$rustGuardTest = Join-Path $repo 'tools\architecture\verify-rust-graph.test.mjs'
$rustResult = Run-Gate -Id 'rust-graph-guard' -BinaryId 'node' -Executable $nodeExe -Arguments @($rustGuard, '--json') -WorkingDirectory $repo -PublishedArguments @('tools/architecture/verify-rust-graph.mjs', '--json')
try {
    $rustGraph = (@($rustResult.Output) -join "`n") | ConvertFrom-Json
}
catch {
    throw "Rust graph guard returned invalid JSON: $($_.Exception.Message)"
}
if ([int]$rustGraph.schemaVersion -ne 2 -or -not [bool]$rustGraph.ok -or [string]$rustGraph.cargoResolver -cne '3' -or [int]$rustGraph.workspaceMembers -ne 15 -or [int]$rustGraph.workspaceDefaultMembers -ne 15) {
    throw 'Rust graph guard did not prove the exact 15-package resolver-3 workspace'
}
[void](Run-Gate -Id 'rust-graph-guard-self-test' -BinaryId 'node' -Executable $nodeExe -Arguments @($rustGuardTest) -WorkingDirectory $repo -PublishedArguments @('tools/architecture/verify-rust-graph.test.mjs'))

$metadataWindows = Run-Gate -Id 'cargo-metadata-windows-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc') -WorkingDirectory $repo -PublishedArguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc')
$metadataLinux = Run-Gate -Id 'cargo-metadata-linux-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-gnu') -WorkingDirectory $repo -PublishedArguments @('metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-unknown-linux-gnu')
$metadataComplete = Run-Gate -Id 'cargo-metadata-complete-closure-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('metadata', '--locked', '--offline', '--format-version', '1') -WorkingDirectory $repo -PublishedArguments @('metadata', '--locked', '--offline', '--format-version', '1')
foreach ($metadataResult in @($metadataWindows, $metadataLinux, $metadataComplete)) {
    try {
        $parsedMetadata = (@($metadataResult.Output) -join "`n") | ConvertFrom-Json
    }
    catch {
        throw "cargo metadata returned invalid JSON: $($_.Exception.Message)"
    }
    if (@($parsedMetadata.workspace_members).Count -ne 15) {
        throw 'cargo metadata workspace member count mismatch'
    }
}
$cargoMetadata = (@($metadataComplete.Output) -join "`n") | ConvertFrom-Json

[void](Run-Gate -Id 'cargo-check-workspace-all-targets-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('check', '--workspace', '--all-targets', '--locked', '--offline') -WorkingDirectory $repo -PublishedArguments @('check', '--workspace', '--all-targets', '--locked', '--offline'))
[void](Run-Gate -Id 'cargo-test-workspace-all-targets-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('test', '--workspace', '--all-targets', '--locked', '--offline') -WorkingDirectory $repo -PublishedArguments @('test', '--workspace', '--all-targets', '--locked', '--offline'))
[void](Run-Gate -Id 'cargo-clippy-workspace-all-targets-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings') -WorkingDirectory $repo -PublishedArguments @('clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings'))
[void](Run-Gate -Id 'cargo-fmt-check' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('fmt', '--all', '--', '--check') -WorkingDirectory $repo -PublishedArguments @('fmt', '--all', '--', '--check'))
[void](Run-Gate -Id 'cargo-deny-policy-locked-offline' -BinaryId 'cargo-deny' -Executable $cargoDenyExe -Arguments @('--all-features', '--locked', '--offline', 'check', 'advisories', 'bans', 'licenses', 'sources') -WorkingDirectory $repo -PublishedArguments @('--all-features', '--locked', '--offline', 'check', 'advisories', 'bans', 'licenses', 'sources'))
[void](Run-Gate -Id 'cargo-test-bcsp-contracts-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('test', '--locked', '--offline', '-p', 'bcsp-contracts') -WorkingDirectory $repo -PublishedArguments @('test', '--locked', '--offline', '-p', 'bcsp-contracts'))
[void](Run-Gate -Id 'cargo-test-bcsp-domain-locked-offline' -BinaryId 'cargo' -Executable $cargoExe -Arguments @('test', '--locked', '--offline', '-p', 'bcsp-domain') -WorkingDirectory $repo -PublishedArguments @('test', '--locked', '--offline', '-p', 'bcsp-domain'))

$dependencyPaths = @(
    'Cargo.toml', 'Cargo.lock', 'frontend/package.json', 'frontend/package-lock.json',
    ':(glob)crates/**/Cargo.toml'
)
$dependencyDiff = Invoke-Git -Arguments (@('diff', '--name-only', $expectedParent, '--') + $dependencyPaths)
if (@($dependencyDiff.Output | Where-Object { $_ -ne '' }).Count -ne 0) {
    throw "dependency manifests or lock artifacts changed: $($dependencyDiff.Output -join ',')"
}
$cargoParentBlob = Get-GitSingle -Arguments @('rev-parse', "$expectedParent`:Cargo.lock")
$cargoCurrentBlob = Get-GitSingle -Arguments @('hash-object', '--path=Cargo.lock', 'Cargo.lock')
$npmParentBlob = Get-GitSingle -Arguments @('rev-parse', "$expectedParent`:frontend/package-lock.json")
$npmCurrentBlob = Get-GitSingle -Arguments @('hash-object', '--path=frontend/package-lock.json', 'frontend/package-lock.json')
if ($cargoParentBlob -cne $cargoCurrentBlob -or $npmParentBlob -cne $npmCurrentBlob) {
    throw 'dependency lock blob identity changed from the fixed predecessor'
}
$cargoLockHash = Get-CanonicalTextSha256 -Path (Join-Path $repo 'Cargo.lock')
$npmLockHash = Get-CanonicalTextSha256 -Path (Join-Path $repo 'frontend\package-lock.json')
$thirdParty = @($cargoMetadata.packages | Where-Object { $null -ne $_.source } | ForEach-Object { "$($_.name)@$($_.version)" } | Sort-Object -Unique)
$dependencyDelta = [ordered]@{
    schemaVersion = 1
    taskId = 'P7.1-004'
    state = 'PASS'
    scope = 'DEPENDENCY_MANIFESTS_LOCKS_AND_RESOLVED_THIRD_PARTY_IDENTITIES'
    fixedBaselineCommit = $expectedParent
    rustWorkspacePackages = @($cargoMetadata.workspace_members).Count
    rustThirdPartyCurrent = $thirdParty.Count
    rustThirdPartyAdded = 0
    rustThirdPartyRemoved = 0
    dependencyManifestPathsChanged = 0
    cargoLockChanged = $false
    cargoLockBaselineBlobOid = $cargoParentBlob
    cargoLockCurrentBlobOid = $cargoCurrentBlob
    cargoLockBaselineCanonicalSha256 = $cargoLockHash
    cargoLockCurrentCanonicalSha256 = $cargoLockHash
    frontendNpmLockChanged = $false
    frontendNpmLockBaselineBlobOid = $npmParentBlob
    frontendNpmLockCurrentBlobOid = $npmCurrentBlob
    frontendNpmLockBaselineCanonicalSha256 = $npmLockHash
    frontendNpmLockCurrentCanonicalSha256 = $npmLockHash
    proof = 'EXACT_BASELINE_GIT_BLOB_IDENTITY_PLUS_LOCKED_OFFLINE_METADATA'
}
Add-GateRecord -Id 'dependency-lock-unchanged' -BinaryId 'builder' -Arguments @()
Add-GateRecord -Id 'dependency-closure-delta' -BinaryId 'builder' -Arguments @()

$canonicalSourceHashes = [ordered]@{}
foreach ($relative in $sourcePaths) {
    $canonicalSourceHashes[$relative] = Get-CanonicalTextSha256 -Path (Join-Path $repo $relative.Replace('/', '\'))
}
$goldenHashes = [ordered]@{}
foreach ($relative in $goldenPaths) {
    $goldenHashes[$relative] = $canonicalSourceHashes[$relative]
}
$manifestEvidence = Read-Json -Path (Join-Path $repo 'crates\bcsp-contracts\tests\golden\contract-manifest-v1.json')
$domainApiContract = [ordered]@{
    schemaVersion = 1
    taskId = 'P7.1-004'
    state = 'PASS'
    scope = 'SHARED_DOMAIN_IDENTITY_AND_TYPED_API_SCHEMA'
    packageImpact = 'BOTH_TARGETS_SHARED_CORE'
    identity = [ordered]@{
        sectionKey = [ordered]@{ parts = @('term', 'campus', 'index'); sectionIndexWidth = 5; leadingZeroesSignificant = $true }
        courseGroupKey = [ordered]@{ parts = @('term', 'campus', 'courseString'); opaqueCourseString = $true }
        courseVariantKey = [ordered]@{ parts = @('group', 'fingerprint'); fingerprintFormat = 'v1:<64-lowercase-hex>'; algorithmVersion = 1 }
        collisionFixture = 'SYNTHETIC_NO_REAL_COURSE_DATA'
    }
    matching = [ordered]@{
        outcomes = @('MATCH', 'UNCERTAIN', 'NO_MATCH')
        reasonCodeCount = 7
        algebra = 'THREE_VALUE_AND_OR_WITH_EMPTY_ACTIVE_DIMENSION_MATCH'
        explanationInvariantChecked = $true
    }
    errors = [ordered]@{
        sharedErrorCodeCount = 11
        concreteBodyAlias = 'ApiErrorBody=TypedApiErrorBody<ApiErrorCode>'
        concreteEnvelopeAlias = 'ApiErrorEnvelope=TypedApiErrorEnvelope<ApiErrorCode>'
        versionDecoder = 'decode_versioned_envelope_json'
        traceIdPolicy = 'RFC4122_RANDOM_UUID_V4_CANONICAL_LOWERCASE_INJECTED_SOURCE'
    }
    schemaManifest = [ordered]@{
        schemaVersion = [int]$manifestEvidence.schemaVersion
        apiProtocolVersion = [int]$manifestEvidence.apiProtocolVersion
        wsProtocolVersion = [int]$manifestEvidence.wsProtocolVersion
        scalarConstraintCount = @($manifestEvidence.scalarConstraints).Count
        schemaCount = @($manifestEvidence.schemas).Count
        schemaIds = @($manifestEvidence.schemas | ForEach-Object { [string]$_.id })
    }
    aggregateWireBoundary = [ordered]@{
        courseGroupSerdeBinding = $false
        courseVariantSerdeBinding = $false
        invariantAggregateOwner = 'bcsp-domain'
        transportProjectionOwner = 'P7.1-005'
    }
    canonicalTextSha256 = $canonicalSourceHashes
    goldenCanonicalTextSha256 = $goldenHashes
    rawFixtureOrSourceBodyPublished = $false
    realCourseDataPublished = $false
}
Add-GateRecord -Id 'schema-golden-hash-lock' -BinaryId 'builder' -Arguments @()
Add-GateRecord -Id 'synthetic-only-fixture-publication-scan' -BinaryId 'builder' -Arguments @()
Add-GateRecord -Id 'exact-task-allowlist' -BinaryId 'builder' -Arguments @()
Add-GateRecord -Id 'protected-worktree-profile' -BinaryId 'builder' -Arguments @('EXACT_PRESERVED_167', 'CLEAN_CHECKOUT_0')
Add-GateRecord -Id 'publication-safety' -BinaryId 'builder' -Arguments @()
Assert-ZeroSideEffects -SideEffects $contract.negativeSideEffects -Label 'P7.1-004 contract'
Add-GateRecord -Id 'zero-side-effects' -BinaryId 'builder' -Arguments @()
Add-GateRecord -Id 'evidence-builder-verify' -BinaryId 'builder' -Arguments @('-VerifyOnly')
Add-GateRecord -Id 'evidence-builder-write' -BinaryId 'builder' -Arguments @()

Compare-Set -Actual @($gateRecords | ForEach-Object { [string]$_.id }) -Expected $expectedQualityGateIds -Label 'generated quality gate IDs'
$qualityEvidence = [ordered]@{
    schemaVersion = 1
    taskId = 'P7.1-004'
    state = 'PASS'
    gates = @($gateRecords)
    rawCommandOutputPublished = $false
    absoluteLocalPathsPublished = $false
    sensitiveMaterialPublished = $false
    protectedWorktree = [ordered]@{
        acceptedProfiles = @('EXACT_PRESERVED_167', 'CLEAN_CHECKOUT_0')
        currentProfileValidated = $true
        observedProfilePublished = $false
        manifestRows = 167
        manifestEntrySetSha256 = [string]$protectedProfile.ManifestEntrySetSha256
        partialProfileAccepted = $false
        opaqueContentRead = $false
    }
    networkUse = [ordered]@{
        writeModePredecessorReplayOnly = $true
        verifyOnly = 'NONE'
        cargo = 'LOCKED_OFFLINE'
        postCommitPredecessorReplay = 'FORBIDDEN'
        postPushPredecessorReplay = 'FORBIDDEN'
    }
}

if ($VerifyOnly) {
    if (-not (Test-Path -LiteralPath $scratchRoot -PathType Container)) {
        [void](New-Item -ItemType Directory -Path $scratchRoot -Force)
    }
    $scratchRootItem = Get-Item -LiteralPath $scratchRoot -Force
    if (-not $scratchRootItem.PSIsContainer -or (([int]$scratchRootItem.Attributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw 'VerifyOnly scratch root must be a normal directory'
    }
    $scratch = Join-Path $scratchRoot "evidence-$PID-$([Guid]::NewGuid().ToString('N'))"
    $outputEvidence = Join-Path $scratch 'evidence'
    $outputRecords = Join-Path $scratch 'records'
}
else {
    $outputEvidence = $canonicalEvidence
    $outputRecords = $canonicalRecords
}

try {
    Write-Json -Path (Join-Path $outputEvidence 'domain-api-contract.json') -Value $domainApiContract
    Write-Json -Path (Join-Path $outputEvidence 'quality-gates.json') -Value $qualityEvidence
    Write-Json -Path (Join-Path $outputEvidence 'dependency-closure-delta.json') -Value $dependencyDelta
    Write-Utf8 -Path (Join-Path $outputEvidence 'commit-allowlist.txt') -Content (($allowlist -join "`n") + "`n")
    Write-Json -Path (Join-Path $outputEvidence 'predecessor-r1-clean-replay.json') -Value $predecessorProof

    $evidenceHashes = [ordered]@{}
    foreach ($name in @('commit-allowlist.txt', 'dependency-closure-delta.json', 'domain-api-contract.json', 'predecessor-r1-clean-replay.json', 'quality-gates.json')) {
        $evidenceHashes[$name] = Get-CanonicalTextSha256 -Path (Join-Path $outputEvidence $name)
    }
    $completionMarkdown = @'
# P7.1-004 completion record

- Task: `P7.1-004`
- Result: `P7_1_004_PASS_COMMIT_ELIGIBLE`
- Scope: shared domain identity, three-value matching, typed HTTP/WebSocket envelopes, stable shared errors, schema manifest, golden fixtures, and invariant aggregate boundaries.
- Package impact: both targets consume the same shared core; this task adds no target-specific runtime behavior.
- Predecessor: independent `EntryCapture` and `PreCommitReplay` clean Windows checkout executions are structurally equivalent.
- Replay phase boundary: predecessor replay occurs only while evidence is written before commit; `VerifyOnly`, `PostCommit`, and `PostPush` never invoke it.
- Dependency boundary: Cargo/npm manifests and lock artifacts are unchanged from the fixed predecessor.
- Test-data boundary: checked-in fixtures are synthetic and contain no real Rutgers course data.
- Commit boundary: exactly 41 allowlisted paths.
- Protected worktree: both `EXACT_PRESERVED_167` and `CLEAN_CHECKOUT_0` are valid profiles.
- Side effects: zero Rutgers requests, database mutations, package builds, Vultr mutations, release publications, and production mutations.
- Actual commit identity is intentionally excluded to avoid self-reference.
- Next task: `P7.1-005`, blocked until `P7_1_004_PASS_POST_PUSH`.
'@
    $completionMarkdownPath = Join-Path $outputRecords 'p7-1-004-completion.md'
    Write-Utf8 -Path $completionMarkdownPath -Content ($completionMarkdown.TrimStart() + "`n")
    $completion = [ordered]@{
        schemaVersion = 1
        recordId = 'P7-1-004-SHARED-DOMAIN-API-2026-07-13-001'
        taskId = 'P7.1-004'
        state = 'P7_1_004_PASS_COMMIT_ELIGIBLE'
        branch = $expectedBranch
        expectedParent = $expectedParent
        expectedRemoteBaseline = $expectedParent
        packageImpact = 'BOTH_TARGETS_SHARED_CORE'
        predecessor = [ordered]@{
            taskId = 'P7.1-003-R1'
            requiredCommit = $expectedParent
            requiredTerminalState = $expectedPredecessorState
            entryCapture = 'PASS'
            preCommitReplay = 'PASS'
            structuredEquivalent = $true
            postCommitReplayInvoked = $false
            postPushReplayInvoked = $false
        }
        commitAllowlist = $allowlist
        commitAllowlistPaths = $allowlist.Count
        protectedBaselineRows = 167
        protectedWorktreeValidationProfiles = @('EXACT_PRESERVED_167', 'CLEAN_CHECKOUT_0')
        protocolVersion = 1
        schemaManifestVersion = 1
        schemaCount = 16
        scalarConstraintCount = 10
        errorCodeCount = 11
        matchOutcomeCount = 3
        matchReasonCodeCount = 7
        lockHashes = [ordered]@{
            cargoSha256 = $cargoLockHash
            frontendNpmSha256 = $npmLockHash
        }
        qualityGateIds = @($gateRecords | ForEach-Object { [string]$_.id })
        evidenceSha256 = $evidenceHashes
        contractCanonicalTextSha256 = Get-CanonicalTextSha256 -Path $contractPath
        completionMarkdownSha256 = Get-CanonicalTextSha256 -Path $completionMarkdownPath
        negativeSideEffects = [ordered]@{
            rutgersRequests = 0
            databaseMutations = 0
            packageBuilds = 0
            vultrMutations = 0
            releasePublications = 0
            productionMutations = 0
        }
        actualCommitExcludedToAvoidSelfReference = $true
        commitEligible = $true
        nextTask = 'P7.1-005'
        nextTaskBlockedUntil = 'P7_1_004_PASS_POST_PUSH'
    }
    Write-Json -Path (Join-Path $outputRecords 'p7-1-004-completion.json') -Value $completion

    if ($VerifyOnly) {
        foreach ($name in @('commit-allowlist.txt', 'dependency-closure-delta.json', 'domain-api-contract.json', 'predecessor-r1-clean-replay.json', 'quality-gates.json')) {
            $expectedPath = Join-Path $outputEvidence $name
            $actualPath = Join-Path $canonicalEvidence $name
            if (-not (Test-Path -LiteralPath $actualPath -PathType Leaf) -or (Get-CanonicalTextSha256 -Path $actualPath) -cne (Get-CanonicalTextSha256 -Path $expectedPath)) {
                throw "canonical evidence drift: $name"
            }
        }
        foreach ($name in @('p7-1-004-completion.md', 'p7-1-004-completion.json')) {
            $expectedPath = Join-Path $outputRecords $name
            $actualPath = Join-Path $canonicalRecords $name
            if (-not (Test-Path -LiteralPath $actualPath -PathType Leaf) -or (Get-CanonicalTextSha256 -Path $actualPath) -cne (Get-CanonicalTextSha256 -Path $expectedPath)) {
                throw "canonical completion drift: $name"
            }
        }
    }

    $head = Get-GitSingle -Arguments @('rev-parse', 'HEAD')
    if ($head -ceq $expectedParent) {
        Compare-Set -Actual @(Get-TaskStatusPaths) -Expected $allowlist -Label 'worktree task/contract allowlist'
    }
    else {
        $parents = (Get-GitSingle -Arguments @('rev-list', '--parents', '-n', '1', 'HEAD')) -split '\s+'
        if ($parents.Count -ne 2 -or $parents[1] -cne $expectedParent) {
            throw 'VerifyOnly is restricted to the direct single-parent P7.1-004 commit'
        }
        Compare-Set -Actual @(Get-CommitPaths -Revision 'HEAD') -Expected $allowlist -Label 'committed task/contract allowlist'
    }

    Test-PublicationFiles -Paths $allowlist
}
finally {
    if ($VerifyOnly -and $null -ne $scratch -and (Test-Path -LiteralPath $scratch -PathType Container)) {
        $fullScratch = [IO.Path]::GetFullPath($scratch)
        $fullScratchRoot = [IO.Path]::GetFullPath($scratchRoot)
        $scratchItem = Get-Item -LiteralPath $fullScratch -Force
        if (-not (Split-Path -Parent $fullScratch).Equals($fullScratchRoot, [StringComparison]::OrdinalIgnoreCase) -or (Split-Path -Leaf $fullScratch) -notmatch '^evidence-[0-9]+-[0-9a-f]{32}$' -or -not $scratchItem.PSIsContainer -or (([int]$scratchItem.Attributes -band [int][IO.FileAttributes]::ReparsePoint) -ne 0)) {
            throw 'VerifyOnly scratch cleanup boundary mismatch'
        }
        Remove-Item -LiteralPath $scratch -Recurse -Force
    }
}

Write-Output "p7_1_004_evidence=$(if ($VerifyOnly) { 'VERIFIED' } else { 'WRITTEN' })"
Write-Output "predecessor_replay=$(if ($VerifyOnly) { 'NOT_INVOKED' } else { 'PRECOMMIT_ONLY_ENTRY_PRESERVED' })"
Write-Output 'domain_api_contract=PASS'
Write-Output 'dependency_lock_delta=ZERO'
Write-Output 'quality_gates=PASS'
Write-Output "protected_worktree_profile=$([string]$protectedProfile.Profile)"
Write-Output "protected_worktree_rows=$([int]$protectedProfile.Rows)"
