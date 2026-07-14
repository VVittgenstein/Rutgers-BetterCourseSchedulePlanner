[CmdletBinding()]
param(
    [ValidateSet('Verify', 'PreCommit', 'PostCommit', 'PostPush')]
    [string]$Mode = 'Verify',
    [string]$ExpectedCommit = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = Join-Path $repo 'project-governance\current\p7'
$evidence = Join-Path $p7 'evidence\p7-1-005'
$records = Join-Path $p7 'records'
$parentCommit = 'b0eac56850f89fc1a2d0e3d4600bd988e97d748c'
$branch = 'codex/p7-implementation'

function Assert-True {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}

function Read-Json {
    param([string]$Path)
    Assert-True (Test-Path -LiteralPath $Path -PathType Leaf) "Missing required JSON: $Path"
    Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

function Invoke-Git {
    param([string[]]$Arguments)
    Push-Location $repo
    try {
        $output = @(& git @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        Pop-Location
    }
    Assert-True ($exitCode -eq 0) "Git command failed: git $($Arguments -join ' ')"
    @($output | ForEach-Object { [string]$_ })
}

function Assert-SameSet {
    param([string[]]$Actual, [string[]]$Expected, [string]$Label)
    $actualSorted = @($Actual | Sort-Object -Unique)
    $expectedSorted = @($Expected | Sort-Object -Unique)
    Assert-True ($actualSorted.Count -eq $expectedSorted.Count) "$Label count mismatch"
    Assert-True (($actualSorted -join "`n") -ceq ($expectedSorted -join "`n")) "$Label mismatch"
}

$contract = Read-Json (Join-Path $p7 '05a-p7-1-005-shared-catalog-discovery-normalization-storage-observations.json')
$replay = Read-Json (Join-Path $evidence 'catalog-recorded-replay.json')
$dependency = Read-Json (Join-Path $evidence 'dependency-delta.json')
$predecessor = Read-Json (Join-Path $evidence 'predecessor-clean-replay.json')
$quality = Read-Json (Join-Path $evidence 'quality-gates.json')
$completion = Read-Json (Join-Path $records 'p7-1-005-completion.json')
$allowlistPath = Join-Path $evidence 'commit-allowlist.txt'
Assert-True (Test-Path -LiteralPath $allowlistPath -PathType Leaf) 'Missing commit allowlist.'
$allowlist = @(Get-Content -LiteralPath $allowlistPath | Where-Object { $_ -ne '' })

Assert-True ([string]$contract.task -ceq 'P7.1-005') 'Contract task mismatch.'
Assert-True ([string]$contract.parentCommit -ceq $parentCommit) 'Contract parent mismatch.'
Assert-True ([int]$contract.publicationBoundary.taskPathCount -eq 50) 'Contract path count mismatch.'
Assert-True ([bool]$contract.governanceSimplification.predecessorRecoveryHelperRetained -eq $false) 'Recovery helper must remain removed.'
Assert-True ($allowlist.Count -eq 50) 'Allowlist must contain 50 paths.'
Assert-True (@($allowlist | Sort-Object -Unique).Count -eq 50) 'Allowlist contains duplicates.'

foreach ($path in $allowlist) {
    Assert-True (-not [IO.Path]::IsPathRooted($path)) "Allowlist path is not repository-relative: $path"
    Assert-True (-not ($path -match '(^|/)(\.secrets|data/staging|target|\.cache)(/|$)')) "Denied path in allowlist: $path"
    Assert-True (-not ($path -match '(?i)(chat-log|\.sqlite(?:3)?$|\.sqlite-(?:wal|shm)$)')) "Denied artifact in allowlist: $path"
    Assert-True (Test-Path -LiteralPath (Join-Path $repo ($path -replace '/', '\')) -PathType Leaf) "Allowlisted file is missing: $path"
}

$protected = @(Import-Csv -LiteralPath (Join-Path $p7 '01-preserved-worktree-manifest.tsv') -Delimiter "`t")
Assert-True ($protected.Count -eq 167) 'Protected-worktree manifest count mismatch.'
$protectedPaths = @($protected | ForEach-Object { [string]$_.path })
Assert-True (@($allowlist | Where-Object { $protectedPaths -contains $_ }).Count -eq 0) 'Task allowlist overlaps the protected worktree.'

$expectedReplay = [ordered]@{
    scopes = 21; nonemptyScopes = 17; emptyScopes = 4; courses = 10629
    rawSections = 22069; uniqueSectionKeys = 22051; equivalentDuplicateKeys = 18
    conflictingDuplicateKeys = 0; meetings = 30804; instructorAssignments = 19202
    multiObjectGroups = 41; multiVariantGroups = 31; crossVariantSectionOverlap = 0
    deliveryConflicts = 184
}
foreach ($entry in $expectedReplay.GetEnumerator()) {
    Assert-True ([int64]$replay.($entry.Key) -eq [int64]$entry.Value) "Recorded replay mismatch: $($entry.Key)"
}
Assert-True ([string]$replay.verifiedBodyHashSet -ceq 'v1:c5e3db46638e32b33498dedb596e5d608c37b6f9ad0328397a3d7bba82154a14') 'Recorded body hash-set mismatch.'
Assert-True ([string]$replay.normalizedContentHashSet -ceq 'v1:a73f3fb9519a625917226e0ee669351c5351646899f798c5c64f5860a479134e') 'Recorded normalized hash-set mismatch.'
Assert-True ([bool]$replay.publishedRawContent -eq $false) 'Recorded replay must not publish raw content.'

$requiredGateIds = @(
    'rust-graph-guard', 'rust-graph-guard-self-test', 'cargo-fmt-check',
    'cargo-check-workspace-locked-offline', 'cargo-test-workspace-locked-offline',
    'cargo-clippy-workspace-locked-offline', 'cargo-deny-policy-locked-offline',
    'recorded-catalog-replay'
)
Assert-SameSet -Actual @($quality.gates | ForEach-Object { [string]$_.id }) -Expected $requiredGateIds -Label 'Quality gates'
foreach ($gate in @($quality.gates)) {
    Assert-True ([int]$gate.exitCode -eq 0 -and [string]$gate.result -ceq 'PASS') "Failed quality evidence: $($gate.id)"
}

Assert-True (@($dependency.newThirdPartyPackages).Count -eq 0) 'Unexpected third-party dependency package.'
Assert-True ([string]$dependency.cargoDeny -ceq 'PASS') 'Dependency policy did not pass.'
Assert-True ([string]$predecessor.requiredCommit -ceq $parentCommit -and [string]$predecessor.result -ceq 'PASS') 'Predecessor evidence mismatch.'
Assert-True ([string]$completion.productState -ceq 'COMPLETE' -and [string]$completion.qualityState -ceq 'PASS') 'Completion record is not ready.'

if ($Mode -eq 'PreCommit') {
    $staged = @(Invoke-Git @('diff', '--cached', '--name-only', '--diff-filter=ACMRT') | Where-Object { $_ -ne '' })
    Assert-SameSet -Actual $staged -Expected $allowlist -Label 'Staged task paths'
}

if ($Mode -in @('PostCommit', 'PostPush')) {
    $head = @(Invoke-Git @('rev-parse', 'HEAD'))[0]
    Assert-True ([string]::IsNullOrEmpty($ExpectedCommit) -or $head -ceq $ExpectedCommit) 'HEAD does not match ExpectedCommit.'
    $actualBranch = @(Invoke-Git @('branch', '--show-current'))[0]
    Assert-True ($actualBranch -ceq $branch) 'Task branch mismatch.'
    $changed = @(Invoke-Git @('diff', '--name-only', '--diff-filter=ACMRT', "$parentCommit..$head") | Where-Object { $_ -ne '' })
    Assert-SameSet -Actual $changed -Expected $allowlist -Label 'Committed task paths'
}

if ($Mode -eq 'PostPush') {
    $head = @(Invoke-Git @('rev-parse', 'HEAD'))[0]
    $remoteLine = @(Invoke-Git @('ls-remote', '--heads', 'origin', "refs/heads/$branch") | Where-Object { $_ -ne '' })
    Assert-True ($remoteLine.Count -eq 1) 'Remote branch lookup did not return exactly one row.'
    $remoteCommit = ($remoteLine[0] -split '\s+')[0]
    Assert-True ($remoteCommit -ceq $head) 'Remote branch does not point at HEAD.'
    Write-Output 'validation_state=P7_1_005_PASS_POST_PUSH'
}
else {
    Write-Output "validation_state=P7_1_005_PASS_$($Mode.ToUpperInvariant())"
}

Write-Output 'protected_worktree_paths=167'
Write-Output 'task_paths=50'
Write-Output 'raw_course_content_published=0'
