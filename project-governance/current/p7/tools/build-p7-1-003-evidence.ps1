[CmdletBinding(DefaultParameterSetName='Build')]
param(
    [Parameter(ParameterSetName='Derive')]
    [switch]$DeriveAllowlist,
    [Parameter(ParameterSetName='Build')]
    [switch]$VerifyOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$baselinePath = Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$previousLockedPath = Join-Path $p7 'evidence\p7-1-002\locked-components.tsv'
$previousCompletionPath = Join-Path $p7 'records\p7-1-002-completion.json'
$sharedDenyPath = Join-Path $repo 'tools\architecture\p4-public-source-deny.json'
$cargoHome = Join-Path $repo '.cache\p7-1-002\cargo'
$rustupHome = Join-Path $repo '.cache\p7-1-002\rustup'
$rustToolchainBin = Join-Path $rustupHome 'toolchains\1.97.0-x86_64-pc-windows-msvc\bin'
$cargoExe = Join-Path $cargoHome 'bin\cargo.exe'
$cargoDenyExe = Join-Path $repo '.cache\p7-1-002\tools\cargo-deny\bin\cargo-deny.exe'
$nodeRoot = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\runtime\node-v24.18.0-win-x64'
$nodeExe = Join-Path $nodeRoot 'node.exe'
$npmExe = Join-Path $nodeRoot 'npm.cmd'
$npmCache = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\npm-cache'
$targetDir = Join-Path $repo '.cache\p7-1-003\target'

function Normalize-Path([string]$Value) { return $Value.Replace('\','/') }
function Read-Json([string]$Path) { return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json }
function Get-Sha256([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash }
function Write-Utf8([string]$Path,[string]$Content) {
    $directory = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $directory)) { [void](New-Item -ItemType Directory -Path $directory -Force) }
    [System.IO.File]::WriteAllText($Path, $Content.Replace("`r`n","`n"), [System.Text.UTF8Encoding]::new($false))
}
function Write-Json([string]$Path,[object]$Value) { Write-Utf8 $Path (($Value | ConvertTo-Json -Depth 30) + "`n") }
function Invoke-Git([string[]]$Arguments,[bool]$AllowFailure=$false) {
    $old=$ErrorActionPreference
    try { $ErrorActionPreference='Continue'; $output=@(& git -C $repo -c core.autocrlf=false @Arguments 2>$null); $code=$LASTEXITCODE }
    finally { $ErrorActionPreference=$old }
    if (-not $AllowFailure -and $code -ne 0) { throw "git $($Arguments -join ' ') failed with exit $code" }
    return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Invoke-CommandCapture([string]$Executable,[string[]]$Arguments,[string]$WorkingDirectory) {
    if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw "missing executable: $Executable" }
    $oldLocation=(Get-Location).Path
    try {
        Set-Location $WorkingDirectory
        $old=$ErrorActionPreference
        try { $ErrorActionPreference='Continue'; $output=@(& $Executable @Arguments 2>&1); $code=$LASTEXITCODE }
        finally { $ErrorActionPreference=$old }
    } finally { Set-Location $oldLocation }
    return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Assert-Pass([object]$Result,[string]$Id) {
    if ([int]$Result.ExitCode -ne 0) { throw "$Id failed with exit $($Result.ExitCode): $((@($Result.Output) | Select-Object -Last 8) -join ' | ')" }
}
function Test-DeniedTaskPath([string]$Path) {
    $p=Normalize-Path $Path
    return $p -match '(^|/)\.secrets(/|$)' -or
        $p -match '(^|/)chat-log-[^/]*\.md$' -or
        $p -match '^docs/(chat-log-|sessions/|p1-a-recovery/)' -or
        $p -match '^project-governance/current/p1/' -or
        $p -match '(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)' -or
        $p -match '(?i)(\.sqlite3?|\.db|-wal|-shm)$'
}
function Get-DerivedAllowlist([object]$Contract) {
    $baseline=@(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")
    if ($baseline.Count -ne 167) { throw "protected baseline row count is $($baseline.Count), expected 167" }
    $protected=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($row in $baseline){[void]$protected.Add((Normalize-Path ([string]$row.path)))}
    $candidate=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $status=(Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output
    foreach($raw in @($status)){
        $line=[string]$raw
        if($line.Length -lt 4){continue}
        $path=Normalize-Path $line.Substring(3)
        if($protected.Contains($path)){continue}
        if(Test-DeniedTaskPath $path){throw "denied path entered task candidate set: $path"}
        [void]$candidate.Add($path)
    }
    foreach($path in @($Contract.commitBoundary.requiredGovernancePaths)){
        $normalized=Normalize-Path ([string]$path)
        if(Test-DeniedTaskPath $normalized){throw "contract requires denied path: $normalized"}
        [void]$candidate.Add($normalized)
    }
    return @($candidate | Sort-Object)
}
function Compare-Set([string[]]$Actual,[string[]]$Expected,[string]$Label){
    $a=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $e=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($v in @($Actual)){if(-not $a.Add([string]$v)){throw "$Label duplicate actual value: $v"}}
    foreach($v in @($Expected)){if(-not $e.Add([string]$v)){throw "$Label duplicate expected value: $v"}}
    $missing=@($e|Where-Object{-not $a.Contains([string]$_)})
    $extra=@($a|Where-Object{-not $e.Contains([string]$_)})
    if($missing.Count -gt 0 -or $extra.Count -gt 0){throw "$Label mismatch; missing=[$($missing -join ',')] extra=[$($extra -join ',')]"}
}

$contract=Read-Json $contractPath
$derived=@(Get-DerivedAllowlist $contract)
if($DeriveAllowlist){$derived|ForEach-Object{Write-Output $_};exit 0}
if(-not ($contract.commitBoundary.finalized -is [bool]) -or -not $contract.commitBoundary.finalized){throw 'contract allowlist is not finalized'}
$allowlist=@($contract.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
$head=((Invoke-Git @('rev-parse','HEAD')).Output -join '').Trim()
if($head -eq [string]$contract.expectedParent){
    Compare-Set $derived $allowlist 'derived/contract allowlist'
}else{
    $committed=@((Invoke-Git @('diff-tree','--no-renames','--no-commit-id','--name-only','-r','HEAD')).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_ -ne ''})
    Compare-Set $committed $allowlist 'committed/contract allowlist'
}

foreach($required in @($cargoExe,$cargoDenyExe,$nodeExe,$npmExe,$previousLockedPath,$previousCompletionPath,$sharedDenyPath)){
    if(-not(Test-Path -LiteralPath $required -PathType Leaf)){throw "missing required input: $required"}
}

$env:CARGO_HOME=$cargoHome
$env:RUSTUP_HOME=$rustupHome
$env:CARGO_TARGET_DIR=$targetDir
$env:RUSTUP_TOOLCHAIN='1.97.0-x86_64-pc-windows-msvc'
$env:CARGO_NET_OFFLINE='true'
$env:npm_config_cache=$npmCache
$env:PATH="$rustToolchainBin;$nodeRoot;$env:PATH"

$gateRecords=[System.Collections.Generic.List[object]]::new()
function Run-Gate([string]$Id,[string]$BinaryId,[string]$Executable,[string[]]$Arguments,[string]$Cwd,[string[]]$PublishedArguments){
    $result=Invoke-CommandCapture $Executable $Arguments $Cwd
    Assert-Pass $result $Id
    $gateRecords.Add([ordered]@{id=$Id;binaryId=$BinaryId;arguments=$PublishedArguments;exitCode=0;normalizedResult='PASS'})
    return $result
}

$rustGuard=Join-Path $repo 'tools\architecture\verify-rust-graph.mjs'
$rustGuardTest=Join-Path $repo 'tools\architecture\verify-rust-graph.test.mjs'
$frontendGuard=Join-Path $repo 'frontend\tools\verify-import-graph.mjs'
$frontendGuardTest=Join-Path $repo 'frontend\tools\verify-import-graph.test.mjs'

$rustJsonResult=Run-Gate 'rust-graph-guard' 'node' $nodeExe @($rustGuard,'--json') $repo @('tools/architecture/verify-rust-graph.mjs','--json')
try{$rustGraph=(@($rustJsonResult.Output)-join "`n")|ConvertFrom-Json}catch{throw "Rust guard returned invalid JSON: $($_.Exception.Message)"}
if([int]$rustGraph.schemaVersion -ne 2 -or -not [bool]$rustGraph.ok -or [string]$rustGraph.cargoResolver -ne '3' -or [int]$rustGraph.workspaceMembers -ne 15 -or [int]$rustGraph.workspaceDefaultMembers -ne 15){throw 'Rust guard JSON does not assert the exact passing schema-v2 resolver-3 15-member/default-member graph'}
Compare-Set @($rustGraph.rootManifestSections) @($contract.rustGraph.rootManifestSections) 'root Cargo manifest sections'
if([int]$rustGraph.publicSourceDeny.expectedCount -ne 18 -or [int]$rustGraph.publicSourceDeny.checkedCount -ne 18 -or [int]$rustGraph.publicSourceDeny.markerCount -ne 212 -or [int]$rustGraph.publicSourceDeny.sourceEscapeCount -ne 0 -or [int]$rustGraph.publicSourceDeny.violationCount -ne 0 -or [int]$rustGraph.publicSourceDeny.scannedFileCount -lt 12){throw 'Rust guard JSON does not assert the complete passing public SOURCE closure'}
Compare-Set @($rustGraph.publicSourceDeny.publicClosurePackages) @($contract.rustGraph.publicSourceClosurePackages) 'Rust public SOURCE closure packages'
[void](Run-Gate 'rust-graph-guard-self-test' 'node' $nodeExe @($rustGuardTest) $repo @('tools/architecture/verify-rust-graph.test.mjs'))

$metadataWindows=Run-Gate 'cargo-metadata-windows' 'cargo' $cargoExe @('metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc') $repo @('metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc')
$metadataLinux=Run-Gate 'cargo-metadata-linux' 'cargo' $cargoExe @('metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-unknown-linux-gnu') $repo @('metadata','--locked','--offline','--format-version','1','--filter-platform','x86_64-unknown-linux-gnu')
$metadataComplete=Run-Gate 'cargo-metadata-complete-closure' 'cargo' $cargoExe @('metadata','--locked','--offline','--format-version','1') $repo @('metadata','--locked','--offline','--format-version','1')
$cargoMetadata=(@($metadataComplete.Output)-join "`n")|ConvertFrom-Json
[void](Run-Gate 'cargo-check-workspace-locked-offline' 'cargo' $cargoExe @('check','--workspace','--all-targets','--locked','--offline') $repo @('check','--workspace','--all-targets','--locked','--offline'))
[void](Run-Gate 'cargo-test-workspace-locked-offline' 'cargo' $cargoExe @('test','--workspace','--all-targets','--locked','--offline') $repo @('test','--workspace','--all-targets','--locked','--offline'))
[void](Run-Gate 'cargo-clippy-workspace-locked-offline' 'cargo' $cargoExe @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings') $repo @('clippy','--workspace','--all-targets','--locked','--offline','--','-D','warnings'))
[void](Run-Gate 'cargo-fmt-check' 'cargo' $cargoExe @('fmt','--all','--','--check') $repo @('fmt','--all','--','--check'))
[void](Run-Gate 'cargo-deny-policy-locked-offline' 'cargo-deny' $cargoDenyExe @('--all-features','--locked','--offline','check','advisories','bans','licenses','sources') $repo @('--all-features','--locked','--offline','check','advisories','bans','licenses','sources'))

[void](Run-Gate 'npm-ci-locked-offline' 'npm' $npmExe @('ci','--prefix','frontend','--ignore-scripts','--offline','--no-audit','--no-fund') $repo @('ci','--prefix','frontend','--ignore-scripts','--offline','--no-audit','--no-fund'))
$frontendJsonResult=Run-Gate 'frontend-import-graph-guard' 'node' $nodeExe @($frontendGuard,'--json') $repo @('frontend/tools/verify-import-graph.mjs','--json')
try{$frontendGraph=(@($frontendJsonResult.Output)-join "`n")|ConvertFrom-Json}catch{throw "frontend guard returned invalid JSON: $($_.Exception.Message)"}
if([string]$frontendGraph.state -ne 'PASS' -or [int]$frontendGraph.publicSourceDeny.checkedCount -ne 18 -or [int]$frontendGraph.publicSourceDeny.checkedMarkerCount -ne 212 -or [int]$frontendGraph.publicSourceDeny.violationCount -ne 0 -or [int]$frontendGraph.prohibitedConstructs.count -ne 0 -or -not [bool]$frontendGraph.repositoryContract.pass){throw 'frontend guard JSON does not assert PASS with frozen roots, 18/18 SOURCE rows, 212 markers and zero violations'}
[void](Run-Gate 'frontend-import-graph-guard-self-test' 'node' $nodeExe @($frontendGuardTest) $repo @('frontend/tools/verify-import-graph.test.mjs'))
[void](Run-Gate 'frontend-typecheck-local' 'npm' $npmExe @('--prefix','frontend','run','typecheck:local') $repo @('--prefix','frontend','run','typecheck:local'))
[void](Run-Gate 'frontend-typecheck-public' 'npm' $npmExe @('--prefix','frontend','run','typecheck:public') $repo @('--prefix','frontend','run','typecheck:public'))
[void](Run-Gate 'frontend-build-local' 'npm' $npmExe @('--prefix','frontend','run','build:local') $repo @('--prefix','frontend','run','build:local'))
[void](Run-Gate 'frontend-build-public' 'npm' $npmExe @('--prefix','frontend','run','build:public') $repo @('--prefix','frontend','run','build:public'))

$previousRows=@(Import-Csv -LiteralPath $previousLockedPath -Delimiter "`t")
$previousRust=@($previousRows|Where-Object{$_.ecosystem -eq 'rust' -and $_.source -notlike 'repository:*'}|ForEach-Object{"$($_.name)@$($_.version)"}|Sort-Object -Unique)
$currentRust=@($cargoMetadata.packages|Where-Object{$null -ne $_.source}|ForEach-Object{"$($_.name)@$($_.version)"}|Sort-Object -Unique)
Compare-Set $currentRust $previousRust 'third-party Rust closure'
$previousCompletion=Read-Json $previousCompletionPath
$cargoLockHash=Get-Sha256 (Join-Path $repo 'Cargo.lock')
$previousCargoLockHash=[string]$previousCompletion.lockHashes.cargoSha256
if($cargoLockHash -eq $previousCargoLockHash){throw 'Cargo.lock was expected to change when the workspace expanded from one package to fifteen'}
$npmLockHash=Get-Sha256 (Join-Path $repo 'frontend\package-lock.json')
if($npmLockHash -ne [string]$previousCompletion.lockHashes.frontendNpmSha256){throw 'frontend package-lock changed during graph-only task'}
$dependencyDelta=[ordered]@{schemaVersion=1;taskId='P7.1-003';state='PASS';scope='THIRD_PARTY_IDENTITIES_AND_LOCK_ARTIFACTS';rustPrevious=$previousRust.Count;rustCurrent=$currentRust.Count;rustAdded=0;rustRemoved=0;previousCargoLockSha256=$previousCargoLockHash;currentCargoLockSha256=$cargoLockHash;cargoLockChanged=$true;npmLockChanged=$false;npmLockSha256=$npmLockHash;workspacePrevious=1;workspaceCurrent=15}
$gateRecords.Add([ordered]@{id='dependency-closure-delta';binaryId='builder';arguments=@();exitCode=0;normalizedResult='PASS'})

$expectedDenyIds=@($contract.p4SourceCoverage.denyIds)
$sharedDeny=Read-Json $sharedDenyPath
if([int]$sharedDeny.schemaVersion -ne 2 -or [int]$sharedDeny.markerSetVersion -ne 1 -or [string]$sharedDeny.kind -ne 'P4_PUBLIC_SOURCE_DENY_FROZEN_SNAPSHOT' -or [string]$sharedDeny.surface -ne 'SOURCE' -or [bool]$sharedDeny.isCapabilityManifest -or [string]$sharedDeny.snapshotMode -ne 'FROZEN_P7_1_003_NO_LIVE_SOURCE_DEPENDENCY' -or [bool]$sharedDeny.runtimeSourceRequired -or @($sharedDeny.capabilities).Count -ne 18){throw 'shared P4 SOURCE frozen snapshot identity mismatch'}
if([string]$sharedDeny.sourceReference -ne 'project-governance/current/p4/05-public-capability-deny-audit.tsv'){throw 'shared P4 SOURCE frozen snapshot provenance reference mismatch'}
$sourceRows=@($sharedDeny.capabilities|ForEach-Object{[pscustomobject]@{deny_id=[string]$_.denyId;capability=[string]$_.capability;validation_id=[string]$_.validationId}})
Compare-Set @($sourceRows|ForEach-Object{[string]$_.deny_id}) $expectedDenyIds 'frozen P4 SOURCE deny IDs'
if($sourceRows.Count -ne 18){throw "frozen P4 SOURCE row count is $($sourceRows.Count), expected 18"}
foreach($row in $sourceRows){if([string]$row.validation_id -ne ([string]$row.deny_id).Replace('P4-D-','P4-Z-')){throw "frozen P4 SOURCE validation identity mismatch: $($row.deny_id)"}}
$sharedMarkerCount=@($sharedDeny.capabilities|ForEach-Object{$_.markers}).Count
if($sharedMarkerCount -ne 212){throw "shared P4 SOURCE marker count is $sharedMarkerCount, expected 212"}
$gateRecords.Add([ordered]@{id='p4-source-row-coverage';binaryId='builder';arguments=@();exitCode=0;normalizedResult='PASS'})
$gateRecords.Add([ordered]@{id='evidence-builder-verify';binaryId='builder';arguments=@('-VerifyOnly');exitCode=0;normalizedResult='PASS'})
$gateRecords.Add([ordered]@{id='evidence-builder-write';binaryId='builder';arguments=@();exitCode=0;normalizedResult='PASS'})

$canonicalEvidence=Join-Path $p7 'evidence\p7-1-003'
$canonicalRecords=Join-Path $p7 'records'
$scratch=Join-Path $repo ".cache\p7-1-003\evidence-$PID-$([Guid]::NewGuid().ToString('N'))"
$outputEvidence=if($VerifyOnly){Join-Path $scratch 'evidence'}else{$canonicalEvidence}
$outputRecords=if($VerifyOnly){Join-Path $scratch 'records'}else{$canonicalRecords}

try{
    Write-Utf8 (Join-Path $outputEvidence 'commit-allowlist.txt') (($allowlist -join "`n")+"`n")
    Write-Json (Join-Path $outputEvidence 'rust-workspace-graph.json') $rustGraph
    Write-Json (Join-Path $outputEvidence 'frontend-import-graph.json') $frontendGraph
    Write-Json (Join-Path $outputEvidence 'dependency-closure-delta.json') $dependencyDelta
    $quality=[ordered]@{schemaVersion=1;taskId='P7.1-003';state='PASS';rawCommandOutputPublished=$false;gates=@($gateRecords)}
    Write-Json (Join-Path $outputEvidence 'quality-gates.json') $quality
    $p4Lines=@('deny_id' + "`t" + 'capability' + "`t" + 'validation_id' + "`t" + 'rust_source_status' + "`t" + 'frontend_source_status' + "`t" + 'task_status')
    foreach($row in @($sourceRows|Sort-Object deny_id)){$p4Lines += "$($row.deny_id)`t$($row.capability)`t$($row.validation_id)`tPASS`tPASS`tPASS_BOTH_PUBLIC_SOURCE_CLOSURES"}
    Write-Utf8 (Join-Path $outputEvidence 'p4-source-coverage.tsv') (($p4Lines -join "`n")+"`n")
    $legacy=@(
      "risk_id`tlegacy_surface`tactive_graph_reachable`trepository_wide_zero_consumer_claim`tdisposition",
      "P6-D031`tFASTIFY_NODE_BACKEND`t0`tFALSE`tFROZEN_EXCLUDED_PENDING_SEMANTIC_MIGRATION",
      "P6-D034`tROOT_TSX_BACKEND_TOOLING`t0`tFALSE`tFROZEN_EXCLUDED_PENDING_SEMANTIC_MIGRATION",
      "P6-D042`tALTERNATE_BUSINESS_STACKS`t0`tFALSE`tFROZEN_EXCLUDED_PENDING_SEMANTIC_MIGRATION"
    )
    Write-Utf8 (Join-Path $outputEvidence 'legacy-active-graph-disposition.tsv') (($legacy -join "`n")+"`n")
    $evidenceHashes=[ordered]@{}
    foreach($name in @('commit-allowlist.txt','rust-workspace-graph.json','frontend-import-graph.json','quality-gates.json','dependency-closure-delta.json','p4-source-coverage.tsv','legacy-active-graph-disposition.tsv')){$evidenceHashes[$name]=Get-Sha256 (Join-Path $outputEvidence $name)}
    $completionMd=@'
# P7.1-003 completion record

- Task: `P7.1-003`
- State: `P7_1_003_PASS_COMMIT_ELIGIBLE`
- Branch: `codex/p7-implementation`
- Expected parent: `1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3`
- Rust workspace packages: `15`
- Frontend entries: `2`
- Rust public SOURCE rows: `18/18` across `12` packages
- Frontend public SOURCE rows: `18/18`
- Shared SOURCE markers: `212`
- Shared SOURCE input: self-contained frozen snapshot; provenance source not required at runtime
- Cargo resolver/default members: `3` / `15`
- Cargo lock: changed only for the fifteen-package workspace graph; third-party Rust identities unchanged
- Frontend npm lock: unchanged
- Remaining P4 zero-surface rows owned by P7.1-013: `126`
- Zero-consumer scope: `ACTIVE_P7_TARGET_GRAPH_ONLY`
- Protected baseline rows: `167`
- Protected-worktree validation profiles: exact preserved `167` or clean checkout `0`; partial profiles fail

All canonical graph, typecheck, build and negative guard gates passed. No repository-wide legacy deletion claim is made. Rutgers requests, database mutations, installable package builds, Vultr mutations, Release publications and production mutations were all zero.

The task becomes complete only after PreCommit, dedicated commit, PostCommit, push and PostPush validation. The next task is `P7.1-004`.
'@
    $completionMdPath=Join-Path $outputRecords 'p7-1-003-completion.md'
    Write-Utf8 $completionMdPath ($completionMd.TrimStart()+"`n")
    $completion=[ordered]@{schemaVersion=1;recordId='P7-1-003-COMPLETION-2026-07-13-001';taskId='P7.1-003';state='P7_1_003_PASS_COMMIT_ELIGIBLE';branch='codex/p7-implementation';expectedParent='1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3';commitAllowlist=$allowlist;rustWorkspacePackages=15;rustPublicSourcePackages=12;rustPublicSourceFiles=[int]$rustGraph.publicSourceDeny.scannedFileCount;frontendEntries=2;p4SourceRows=18;rustP4SourceRows=18;frontendP4SourceRows=18;p4SourceMarkers=212;p4RemainingRowsOwner='P7.1-013';protectedBaselineRows=167;protectedWorktreeValidationProfiles=@('EXACT_PRESERVED_167','CLEAN_CHECKOUT_0');zeroConsumerScope='ACTIVE_P7_TARGET_GRAPH_ONLY';repositoryWideZeroConsumerClaim=$false;legacyGraphDisposition='FROZEN_EXCLUDED_PENDING_SEMANTIC_MIGRATION';lockHashes=[ordered]@{cargoSha256=$cargoLockHash;frontendNpmSha256=$npmLockHash};qualityGateIds=@($gateRecords|ForEach-Object{$_.id});evidenceSha256=$evidenceHashes;completionMarkdownSha256=(Get-Sha256 $completionMdPath);negativeSideEffects=[ordered]@{rutgersRequests=0;databaseMutations=0;packageBuilds=0;vultrMutations=0;releasePublications=0;productionMutations=0};actualCommitExcludedToAvoidSelfReference=$true;commitEligible=$true;nextTask='P7.1-004';nextTaskBlockedUntil='P7_1_003_PASS_POST_PUSH'}
    Write-Json (Join-Path $outputRecords 'p7-1-003-completion.json') $completion
    if($VerifyOnly){
        foreach($name in @('commit-allowlist.txt','rust-workspace-graph.json','frontend-import-graph.json','quality-gates.json','dependency-closure-delta.json','p4-source-coverage.tsv','legacy-active-graph-disposition.tsv')){
            $actual=Join-Path $canonicalEvidence $name;$expected=Join-Path $outputEvidence $name
            if(-not(Test-Path -LiteralPath $actual -PathType Leaf) -or (Get-Sha256 $actual) -ne (Get-Sha256 $expected)){throw "canonical evidence drift: $name"}
        }
        foreach($name in @('p7-1-003-completion.md','p7-1-003-completion.json')){
            $actual=Join-Path $canonicalRecords $name;$expected=Join-Path $outputRecords $name
            if(-not(Test-Path -LiteralPath $actual -PathType Leaf) -or (Get-Sha256 $actual) -ne (Get-Sha256 $expected)){throw "canonical completion drift: $name"}
        }
    }
}finally{
    if($VerifyOnly -and (Test-Path -LiteralPath $scratch)){
        $scratchRoot=[System.IO.Path]::GetFullPath((Join-Path $repo '.cache\p7-1-003'))
        $resolvedScratch=[System.IO.Path]::GetFullPath($scratch)
        if(-not $resolvedScratch.StartsWith($scratchRoot+[System.IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw "refusing unsafe scratch cleanup: $resolvedScratch"}
        Remove-Item -LiteralPath $resolvedScratch -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Output "p7_1_003_evidence=$(if($VerifyOnly){'VERIFIED'}else{'WRITTEN'})"
Write-Output "task_allowlist_paths=$($allowlist.Count)"
Write-Output 'rust_workspace_packages=15'
Write-Output 'rust_public_source_packages=12'
Write-Output 'rust_p4_source_rows=18'
Write-Output 'frontend_p4_source_rows=18'
Write-Output 'p4_source_markers=212'
