[CmdletBinding()]
param(
    [ValidateSet('Capture','PreCommit','PostCommit')]
    [string]$Mode = 'PreCommit'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$authorityPath = Join-Path $p7 '00a-p7-authority-and-scope.json'
$manifestPath = Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$completionPath = Join-Path $p7 'records\p7-1-001-completion.json'
$failures = [System.Collections.Generic.List[string]]::new()

function Add-Failure([string]$Message) { $failures.Add($Message) }
function Assert-True([bool]$Condition, [string]$Message) { if (-not $Condition) { Add-Failure $Message } }
function Normalize-Path([string]$Path) { return $Path.Replace('\','/') }
function Get-Sha256([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash }
function Write-Utf8NoBomLines([string]$Path, [string[]]$Lines) {
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent)) { [IO.Directory]::CreateDirectory($parent) | Out-Null }
    $text = ($Lines -join "`n") + "`n"
    [IO.File]::WriteAllText($Path, $text, [Text.UTF8Encoding]::new($false))
}
function Invoke-Git([string[]]$Arguments, [bool]$AllowFailure = $false) {
    $output = @(& git -C $repo @Arguments 2>&1)
    $code = $LASTEXITCODE
    if (-not $AllowFailure -and $code -ne 0) { throw "git $($Arguments -join ' ') failed with exit $code" }
    return [pscustomobject]@{ Output = $output; ExitCode = $code }
}
function Get-Authority {
    if (-not (Test-Path -LiteralPath $authorityPath)) { throw "Missing authority JSON: $authorityPath" }
    return Get-Content -LiteralPath $authorityPath -Raw -Encoding utf8 | ConvertFrom-Json
}
function Test-ProtectedOpaquePath([string]$Path) {
    return $Path -like 'chat-log-*.md' -or
        $Path -like 'docs/chat-log-*' -or
        $Path -like 'docs/sessions/*' -or
        $Path -like 'docs/p1-a-recovery/*' -or
        $Path -like 'project-governance/current/p1/*'
}
function Get-HeadBlob([string]$Path) {
    $result = Invoke-Git @('rev-parse', "HEAD:$Path")
    return ([string]$result.Output[0]).Trim()
}
function Get-BaselineRecords([object]$Authority) {
    $allowed = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in @($Authority.taskCommitAllowlist)) { [void]$allowed.Add([string]$path) }

    $statusResult = Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')
    $records = [System.Collections.Generic.List[object]]::new()
    $unexpectedP7 = [System.Collections.Generic.List[string]]::new()

    foreach ($rawLine in @($statusResult.Output)) {
        $line = [string]$rawLine
        if ($line.Length -lt 4) { continue }
        $code = $line.Substring(0,2)
        $path = Normalize-Path $line.Substring(3)
        if ($path.StartsWith('"')) { throw "Quoted/special Git path is unsupported by the frozen manifest: $path" }
        if ($path.StartsWith('project-governance/current/p7/', [StringComparison]::Ordinal)) {
            if (-not $allowed.Contains($path)) { $unexpectedP7.Add($path) }
            continue
        }
        if ($code[0] -ne ' ' -and $code -ne '??') { Add-Failure "Pre-existing path is staged and ownership is ambiguous: $path ($code)" }

        $absolute = Join-Path $repo $path.Replace('/','\')
        $opaque = Test-ProtectedOpaquePath $path
        $headBlob = ''
        $sha = ''
        $size = [int64]0
        $kind = ''
        $normalizedStatus = ''
        $contentPolicy = ''

        if ($code -eq '??') {
            $normalizedStatus = 'UNTRACKED'
            if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) { throw "Untracked manifest path is not a file: $path" }
            $size = (Get-Item -LiteralPath $absolute).Length
            if ($opaque) {
                $kind = 'UNTRACKED_OPAQUE'
                $sha = 'NOT_COMPUTED'
                $contentPolicy = 'OPAQUE_PROTECTED_NO_READ'
            } else {
                $kind = 'UNTRACKED_FILE'
                $sha = Get-Sha256 $absolute
                $contentPolicy = 'PHYSICAL_BYTES_SHA256'
            }
        } elseif ($code[1] -eq 'D') {
            $normalizedStatus = 'WORKTREE_D'
            $kind = 'TRACKED_DELETED'
            $headBlob = Get-HeadBlob $path
            $sha = 'ABSENT'
            $contentPolicy = 'HEAD_BLOB_ONLY_NO_WORKTREE_READ'
            if (Test-Path -LiteralPath $absolute) { Add-Failure "Tracked deleted baseline path unexpectedly exists: $path" }
        } else {
            $normalizedStatus = "WORKTREE_$($code[1])"
            $kind = 'TRACKED_MODIFIED'
            $headBlob = Get-HeadBlob $path
            if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) { throw "Tracked dirty path is not a file: $path" }
            $size = (Get-Item -LiteralPath $absolute).Length
            if ($opaque) {
                $sha = 'NOT_COMPUTED'
                $contentPolicy = 'OPAQUE_PROTECTED_NO_READ'
            } else {
                $sha = Get-Sha256 $absolute
                $contentPolicy = 'PHYSICAL_BYTES_SHA256'
            }
        }

        $records.Add([pscustomobject]@{
            path = $path
            git_status = $normalizedStatus
            baseline_kind = $kind
            head_blob_oid = $headBlob
            worktree_sha256 = $sha
            size_bytes = $size
            content_policy = $contentPolicy
            ownership = 'USER_EXISTING'
            required_action = 'PRESERVE'
        })
    }

    Assert-True ($unexpectedP7.Count -eq 0) "Unexpected P7 paths outside the six-file allowlist: $($unexpectedP7 -join ',')"
    return @($records | Sort-Object -Property @{Expression='path'; Ascending=$true})
}
function Convert-RecordsToTsv([object[]]$Records) {
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("path`tgit_status`tbaseline_kind`thead_blob_oid`tworktree_sha256`tsize_bytes`tcontent_policy`townership`trequired_action")
    foreach ($r in $Records) {
        foreach ($value in @($r.path,$r.git_status,$r.baseline_kind,$r.head_blob_oid,$r.worktree_sha256,[string]$r.size_bytes,$r.content_policy,$r.ownership,$r.required_action)) {
            if ([string]$value -match "[`t`r`n]") { throw "Manifest value contains a forbidden control character: $($r.path)" }
        }
        $lines.Add((@($r.path,$r.git_status,$r.baseline_kind,$r.head_blob_oid,$r.worktree_sha256,[string]$r.size_bytes,$r.content_policy,$r.ownership,$r.required_action) -join "`t"))
    }
    return @($lines)
}
function Get-EntrySetSha([string[]]$TsvLines) {
    $canonical = (($TsvLines | Select-Object -Skip 1) -join "`n") + "`n"
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($canonical)
    $hash = [Security.Cryptography.SHA256]::Create().ComputeHash($bytes)
    return ([BitConverter]::ToString($hash)).Replace('-','')
}
function Assert-SameSet([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    $a = @($Actual | Sort-Object -Unique)
    $e = @($Expected | Sort-Object -Unique)
    $diff = @(Compare-Object -ReferenceObject $e -DifferenceObject $a)
    Assert-True ($diff.Count -eq 0) "$Label mismatch: $((@($diff | ForEach-Object { $_.SideIndicator + $_.InputObject })) -join ',')"
}
function Test-Authority([object]$a) {
    Assert-True ([string]$a.recordId -eq 'P7-AUTH-2026-07-13-001') 'authority recordId mismatch'
    Assert-True ([string]$a.state -eq 'P7_AUTHORIZED') 'P7 is not authorized in authority record'
    Assert-True ([string]$a.activeTask -eq 'P7.1-001') 'active task must be P7.1-001'
    Assert-True ([bool]$a.authorization.p71ThroughP74ImplementationAuthorized) 'P7.1-P7.4 implementation authority missing'
    Assert-True (-not [bool]$a.authorization.p75RealWorldExecutionAuthorized) 'P7.5 real-world execution is prematurely authorized'
    Assert-True (-not [bool]$a.authorization.realWorldNetworkTestAuthorized) 'real-world network test is prematurely authorized'
    Assert-True (-not [bool]$a.authorization.vultrStagingMutationAuthorized) 'Vultr staging mutation is prematurely authorized'
    Assert-True (-not [bool]$a.authorization.githubReleaseAuthorized) 'GitHub Release is prematurely authorized'
    Assert-True (-not [bool]$a.authorization.productionAuthorized) 'production is prematurely authorized'
    Assert-True ([string]$a.git.sourceCommit -eq 'a4b035a586a4b14fc3a75698caf99badce869fd5') 'source commit mismatch'
    Assert-True ([string]$a.git.targetBranch -eq 'codex/p7-implementation') 'target branch mismatch'
    Assert-True (-not [bool]$a.git.forcePush) 'force push must remain false'
    Assert-True (-not [bool]$a.git.stashExistingWorktree -and -not [bool]$a.git.resetExistingWorktree -and -not [bool]$a.git.cleanExistingWorktree) 'existing worktree mutation is allowed by authority JSON'
    Assert-True ([int]$a.taskPlan.taskCount -eq 32) 'task count must be 32'
    Assert-True ([int]$a.taskPlan.exactPackageCount -eq 2) 'package count must be two'
    Assert-True (@($a.taskCommitAllowlist).Count -eq 6) 'task commit allowlist must contain six paths'
    Assert-True ([int]$a.pinCount -eq 20 -and @($a.pins).Count -eq 20) 'authority pin count must be 20'
}
function Test-UpstreamPins([object]$a) {
    $pinLines = [System.Collections.Generic.List[string]]::new()
    foreach ($pin in @($a.pins | Sort-Object -Property path)) {
        $relative = [string]$pin.path
        $absolute = Join-Path $repo $relative.Replace('/','\')
        Assert-True (Test-Path -LiteralPath $absolute -PathType Leaf) "missing pinned input: $relative"
        if (Test-Path -LiteralPath $absolute -PathType Leaf) {
            $actual = Get-Sha256 $absolute
            Assert-True ($actual -eq [string]$pin.sha256) "pinned input hash mismatch: $relative"
            $pinLines.Add("$relative`t$actual")
        }
    }
    $canonical = ($pinLines -join "`n") + "`n"
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes($canonical)
    $digest = ([BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash($bytes))).Replace('-','')
    Assert-True ($digest -eq [string]$a.canonicalPinsetSha256) 'canonical upstream pinset digest mismatch'
}
function Test-UpstreamGates {
    $commands = @(
        @{ name='P3'; path='project-governance\current\p3\tools\validate-p3.ps1'; args=@('-ValidationMode','Final'); anchor='p3_gate=P3_PASS' },
        @{ name='P4'; path='project-governance\current\p4\tools\validate-p4.ps1'; args=@('-ValidationMode','Final'); anchor='p4_gate=P4_PASS' },
        @{ name='P5'; path='project-governance\current\p5\tools\validate-p5.ps1'; args=@('-ValidationMode','Final'); anchor='p5_gate=P5_PASS' },
        @{ name='P6'; path='project-governance\current\p6\tools\validate-p6.ps1'; args=@(); anchor='p6_gate=P6_REVIEW_READY' }
    )
    foreach ($c in $commands) {
        $script = Join-Path $repo $c.path
        $out = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $script @($c.args) 2>&1)
        Assert-True ($LASTEXITCODE -eq 0) "$($c.name) validator failed"
        Assert-True ($out -contains $c.anchor) "$($c.name) gate anchor missing"
        Assert-True ($out -contains 'integrity_failures=0') "$($c.name) integrity failures are nonzero or absent"
    }
}
function Test-TaskMatrix {
    $matrixPath = Join-Path $repo 'project-governance\current\p6\04-p7-task-and-commit-matrix.tsv'
    $rows = @(Import-Csv -LiteralPath $matrixPath -Delimiter "`t")
    Assert-True ($rows.Count -eq 32) "task matrix count is $($rows.Count), expected 32"
    Assert-True (@($rows.task_id | Sort-Object -Unique).Count -eq 32) 'task IDs are not unique'
    $first = @($rows | Where-Object { $_.task_id -eq 'P7.1-001' })
    Assert-True ($first.Count -eq 1 -and [string]$first[0].dependencies -eq 'NONE') 'P7.1-001 dependency contract mismatch'
}
function Get-IndexPaths {
    $result = Invoke-Git @('diff','--cached','--name-only','--diff-filter=ACDMRTUXB')
    return @($result.Output | ForEach-Object { Normalize-Path ([string]$_) } | Where-Object { $_ -ne '' })
}
function Test-StagedSecretPatterns([string[]]$Paths, [string]$Source) {
    $patterns = @(
        '-----BEGIN [A-Z ]*PRIVATE KEY-----',
        '(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}',
        'AKIA[0-9A-Z]{16}',
        'gh[pousr]_[A-Za-z0-9]{30,}'
    )
    foreach ($path in $Paths) {
        $spec = if ($Source -eq 'index') { ":$path" } else { "HEAD:$path" }
        $content = ((Invoke-Git @('show',$spec)).Output -join "`n")
        foreach ($pattern in $patterns) { Assert-True (-not [regex]::IsMatch($content,$pattern)) "high-confidence secret pattern in $Source blob: $path" }
    }
}

Push-Location $repo
try {
    $authority = Get-Authority
    Test-Authority $authority
    Test-UpstreamPins $authority
    Test-TaskMatrix

    $branch = ((Invoke-Git @('branch','--show-current')).Output -join '').Trim()
    Assert-True ($branch -eq [string]$authority.git.targetBranch) "active branch mismatch: $branch"
    $head = ((Invoke-Git @('rev-parse','HEAD')).Output -join '').Trim()
    if ($Mode -in @('Capture','PreCommit')) { Assert-True ($head -eq [string]$authority.git.sourceCommit) "pre-commit HEAD mismatch: $head" }

    $records = Get-BaselineRecords $authority
    $tsvLines = Convert-RecordsToTsv $records
    $entrySetSha = Get-EntrySetSha $tsvLines

    if ($Mode -eq 'Capture') {
        Assert-True (@(Get-IndexPaths).Count -eq 0) 'index must be empty during baseline capture'
        Assert-True ($records.Count -eq [int]$authority.worktreeBaseline.preexistingDirtyPaths) "baseline path count is $($records.Count), expected $($authority.worktreeBaseline.preexistingDirtyPaths)"
        if ($failures.Count -eq 0) { Write-Utf8NoBomLines $manifestPath $tsvLines }
    } else {
        Assert-True (Test-Path -LiteralPath $manifestPath -PathType Leaf) 'preserved-worktree manifest is missing'
        if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
            $frozen = @(Get-Content -LiteralPath $manifestPath -Encoding utf8)
            Assert-True ($frozen.Count -eq $tsvLines.Count) 'preserved-worktree manifest line count drifted'
            if ($frozen.Count -eq $tsvLines.Count) {
                for ($i=0; $i -lt $frozen.Count; $i++) { if ($frozen[$i] -cne $tsvLines[$i]) { Add-Failure "preserved-worktree drift at manifest line $($i+1)"; break } }
            }
        }
        Assert-True (Test-Path -LiteralPath $completionPath -PathType Leaf) 'completion JSON is missing'
        if (Test-Path -LiteralPath $completionPath -PathType Leaf) {
            $completion = Get-Content -LiteralPath $completionPath -Raw -Encoding utf8 | ConvertFrom-Json
            Assert-True ([string]$completion.taskId -eq 'P7.1-001') 'completion taskId mismatch'
            Assert-True ([string]$completion.state -eq 'P7_1_001_PASS_COMMIT_ELIGIBLE') 'completion state mismatch'
            Assert-True ([string]$completion.expectedParent -eq [string]$authority.git.sourceCommit) 'completion expected parent mismatch'
            Assert-True ([int]$completion.manifestRows -eq $records.Count) 'completion manifest row count mismatch'
            Assert-True ([string]$completion.manifestEntrySetSha256 -eq $entrySetSha) 'completion manifest entry-set digest mismatch'
            Assert-SameSet @($completion.commitAllowlist) @($authority.taskCommitAllowlist) 'completion commit allowlist'
        }
    }

    $ignore = Invoke-Git @('check-ignore','-q','.secrets/') $true
    Assert-True ($ignore.ExitCode -eq 0) '.secrets/ is not ignored'
    $trackedPrivate = @((Invoke-Git @('ls-files','--','.secrets')).Output | Where-Object { $_ -ne '' })
    Assert-True ($trackedPrivate.Count -eq 0) '.secrets/ contains tracked paths'

    if ($Mode -eq 'PreCommit') {
        Test-UpstreamGates
        $indexPaths = Get-IndexPaths
        Assert-SameSet $indexPaths @($authority.taskCommitAllowlist) 'staged path allowlist'
        $nameStatus = @((Invoke-Git @('diff','--cached','--name-status')).Output)
        Assert-True (@($nameStatus | Where-Object { $_ -notmatch '^A\s+' }).Count -eq 0) 'all staged task paths must be additions'
        Test-StagedSecretPatterns $indexPaths 'index'
    }

    if ($Mode -eq 'PostCommit') {
        $parent = ((Invoke-Git @('rev-parse','HEAD^')).Output -join '').Trim()
        Assert-True ($parent -eq [string]$authority.git.sourceCommit) "task commit parent mismatch: $parent"
        $committed = @((Invoke-Git @('diff-tree','--no-commit-id','--name-only','-r','HEAD')).Output | ForEach-Object { Normalize-Path ([string]$_) } | Where-Object { $_ -ne '' })
        Assert-SameSet $committed @($authority.taskCommitAllowlist) 'committed path allowlist'
        $commitStatus = @((Invoke-Git @('diff-tree','--no-commit-id','--name-status','-r','HEAD')).Output)
        Assert-True (@($commitStatus | Where-Object { $_ -notmatch '^A\s+' }).Count -eq 0) 'all committed task paths must be additions'
        Assert-True (@(Get-IndexPaths).Count -eq 0) 'index must be empty after task commit'
        Test-StagedSecretPatterns $committed 'HEAD'
    }
} finally {
    Pop-Location
}

if ($failures.Count -gt 0) {
    Write-Output 'p7_1_001_gate=FAIL'
    $failures | ForEach-Object { Write-Output "failure=$_" }
    exit 1
}

Write-Output "validation_mode=$Mode"
Write-Output 'p7_authority=P7_AUTHORIZED'
Write-Output 'active_task=P7.1-001'
Write-Output "manifest_rows=$($records.Count)"
Write-Output "manifest_entry_set_sha256=$entrySetSha"
Write-Output 'preexisting_worktree=PRESERVED'
Write-Output 'staged_foreign_paths=0'
Write-Output 'rutgers_requests=0'
Write-Output 'product_source_mutations=0'
Write-Output 'dependency_mutations=0'
Write-Output 'product_builds=0'
Write-Output 'package_builds=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
if ($Mode -eq 'PostCommit') { Write-Output "task_commit=$head" }
Write-Output 'p7_1_001_gate=PASS'
