[CmdletBinding(DefaultParameterSetName='Build')]
param(
    [Parameter(ParameterSetName='Derive')]
    [switch]$DeriveAllowlist,
    [Parameter(ParameterSetName='Build')]
    [switch]$VerifyOnly
)

$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest

$repo=(Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath=Join-Path $p7 '03r1a-p7-1-003-r1-windows-checkout-portability-repair.json'
$baselinePath=Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$primaryContractPath=Join-Path $p7 '03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$primaryBuilderPath=Join-Path $p7 'tools\build-p7-1-003-evidence.ps1'
$canonicalEvidence=Join-Path $p7 'evidence\p7-1-003-r1'
$canonicalRecords=Join-Path $p7 'records'
$cacheRoot=Join-Path $repo '.cache\p7-1-003-r1'
$primaryCommit='c2af7184aeec06704997f266530535003358c278'
$primaryParent='1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3'
$expectedBranch='codex/p7-implementation'
$primaryContractRelative='project-governance/current/p7/03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$primaryAllowlistRelative='project-governance/current/p7/evidence/p7-1-003/commit-allowlist.txt'
$primaryCompletionRelative='project-governance/current/p7/records/p7-1-003-completion.json'
$expectedRepairAllowlist=@(
    'project-governance/current/p7/03r1-p7-1-003-r1-windows-checkout-portability-repair.md',
    'project-governance/current/p7/03r1a-p7-1-003-r1-windows-checkout-portability-repair.json',
    'project-governance/current/p7/evidence/p7-1-003-r1/checkout-portability.json',
    'project-governance/current/p7/evidence/p7-1-003-r1/commit-allowlist.txt',
    'project-governance/current/p7/records/p7-1-003-r1-completion.json',
    'project-governance/current/p7/records/p7-1-003-r1-completion.md',
    'project-governance/current/p7/tools/build-p7-1-003-evidence.ps1',
    'project-governance/current/p7/tools/build-p7-1-003-r1-evidence.ps1',
    'project-governance/current/p7/tools/validate-p7-1-003-r1.ps1',
    'project-governance/current/p7/tools/validate-p7-1-003.ps1'
)
$expectedRequiredGovernance=@(
    'project-governance/current/p7/03r1-p7-1-003-r1-windows-checkout-portability-repair.md',
    'project-governance/current/p7/03r1a-p7-1-003-r1-windows-checkout-portability-repair.json',
    'project-governance/current/p7/evidence/p7-1-003-r1/checkout-portability.json',
    'project-governance/current/p7/evidence/p7-1-003-r1/commit-allowlist.txt',
    'project-governance/current/p7/records/p7-1-003-r1-completion.json',
    'project-governance/current/p7/records/p7-1-003-r1-completion.md',
    'project-governance/current/p7/tools/build-p7-1-003-r1-evidence.ps1',
    'project-governance/current/p7/tools/validate-p7-1-003-r1.ps1'
)
$expectedQualityGates=@('immutable-primary-boundary','repair-commit-boundary','effective-checkout-status','canonical-text-hash-policy','autocrlf-true-isolated-clone-self-test','primary-p7-builder-verify','protected-worktree-profile','publication-safety','repair-evidence-builder-verify','fast-forward-remote-boundary')
$gateDispositions=[ordered]@{
    'immutable-primary-boundary'='PASS'
    'repair-commit-boundary'='REQUIRED_AT_TRANSACTION'
    'effective-checkout-status'='PASS'
    'canonical-text-hash-policy'='PASS'
    'autocrlf-true-isolated-clone-self-test'='PASS'
    'primary-p7-builder-verify'='PASS'
    'protected-worktree-profile'='REQUIRED_AT_VALIDATOR'
    'publication-safety'='REQUIRED_AT_VALIDATOR'
    'repair-evidence-builder-verify'='REQUIRED_AT_VALIDATOR'
    'fast-forward-remote-boundary'='REQUIRED_AT_TRANSACTION'
}

function Normalize-Path([string]$Value){return $Value.Replace('\','/')}
function Read-Json([string]$Path){return Get-Content -LiteralPath $Path -Raw -Encoding utf8|ConvertFrom-Json}
function Write-Utf8([string]$Path,[string]$Content){
    $directory=Split-Path -Parent $Path;if(-not(Test-Path -LiteralPath $directory)){[void](New-Item -ItemType Directory -Path $directory -Force)}
    [System.IO.File]::WriteAllText($Path,$Content.Replace("`r`n","`n"),[System.Text.UTF8Encoding]::new($false))
}
function Write-Json([string]$Path,[object]$Value){Write-Utf8 $Path (($Value|ConvertTo-Json -Depth 30)+"`n")}
function Get-RawSha256([string]$Path){return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash}
function Get-CanonicalTextInfo([string]$Path){
    $bytes=[System.IO.File]::ReadAllBytes($Path);$utf8=[System.Text.UTF8Encoding]::new($false,$true);$text=$utf8.GetString($bytes)
    if($text.Length-gt0-and$text[0]-eq[char]0xFEFF){throw "UTF-8 BOM is forbidden in canonical text: $Path"}
    if($text.Contains([char]0)){throw "NUL is forbidden in canonical text: $Path"}
    $withoutPairs=$text.Replace("`r`n",'');if($withoutPairs.Contains("`r")){throw "lone CR is forbidden in canonical text: $Path"}
    $crlfCount=[regex]::Matches($text,"`r`n").Count;$canonical=$text.Replace("`r`n","`n")
    if(-not$canonical.EndsWith("`n")){throw "canonical text must end with LF: $Path"}
    $canonicalBytes=[System.Text.UTF8Encoding]::new($false).GetBytes($canonical);$hasher=[System.Security.Cryptography.SHA256]::Create()
    try{$canonicalHash=$hasher.ComputeHash($canonicalBytes)}finally{$hasher.Dispose()}
    return [pscustomobject]@{rawSha256=(Get-RawSha256 $Path);canonicalSha256=([System.BitConverter]::ToString($canonicalHash)).Replace('-','');crlfCount=$crlfCount;canonicalBytes=$canonicalBytes.Length}
}
function Invoke-Git([string[]]$Arguments,[bool]$AllowFailure=$false){
    $old=$ErrorActionPreference;try{$ErrorActionPreference='Continue';$output=@(& git -C $repo @Arguments 2>$null);$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
    if(-not$AllowFailure-and$code-ne0){throw "git $($Arguments-join' ') failed with exit $code"};return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Invoke-GitAt([string]$Directory,[string[]]$Arguments,[bool]$AllowFailure=$false){
    $old=$ErrorActionPreference;try{$ErrorActionPreference='Continue';$output=@(& git -C $Directory @Arguments 2>$null);$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
    if(-not$AllowFailure-and$code-ne0){throw "fixture git $($Arguments-join' ') failed with exit $code"};return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Compare-Set([string[]]$Actual,[string[]]$Expected,[string]$Label){
    $a=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$e=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($value in @($Actual)){if(-not$a.Add([string]$value)){throw "$Label duplicate actual value: $value"}}
    foreach($value in @($Expected)){if(-not$e.Add([string]$value)){throw "$Label duplicate expected value: $value"}}
    $missing=@($e|Where-Object{-not$a.Contains([string]$_)});$extra=@($a|Where-Object{-not$e.Contains([string]$_)})
    if($missing.Count-gt0-or$extra.Count-gt0){throw "$Label mismatch; missing=[$($missing-join',')] extra=[$($extra-join',')]"}
}
function Test-DeniedTaskPath([string]$Path){
    $p=Normalize-Path $Path
    return $p-match'(?i)(^|/)\.secrets(/|$)'-or$p-match'(?i)(^|/)chat-log-[^/]*\.md$'-or$p-match'(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)'-or$p-match'(?i)^project-governance/current/p1/'-or$p-match'(?i)(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)'-or$p-match'(?i)(\.sqlite3?|\.db|-wal|-shm)$'
}
function Get-DerivedAllowlist([object]$Contract){
    $baseline=@(Import-Csv -LiteralPath $baselinePath -Delimiter "`t");if($baseline.Count-ne167){throw "protected baseline row count is $($baseline.Count), expected 167"}
    $protected=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($row in $baseline){[void]$protected.Add((Normalize-Path([string]$row.path)))}
    $candidate=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($raw in @((Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output)){
        $line=[string]$raw;if($line.Length-lt4){continue};$path=Normalize-Path $line.Substring(3);if($protected.Contains($path)){continue};if(Test-DeniedTaskPath $path){throw "denied path entered repair candidate set: $path"};[void]$candidate.Add($path)
    }
    foreach($path in @($Contract.commitBoundary.requiredGovernancePaths)){$normalized=Normalize-Path([string]$path);if(Test-DeniedTaskPath $normalized){throw "repair contract requires denied path: $normalized"};[void]$candidate.Add($normalized)}
    return @($candidate|Sort-Object)
}
function Assert-PrimaryBoundary([object]$RepairContract){
    $exists=Invoke-Git @('cat-file','-e',"$primaryCommit^{commit}") $true;if($exists.ExitCode-ne0){throw 'immutable primary commit is unavailable'}
    $parents=((Invoke-Git @('rev-list','--parents','-n','1',$primaryCommit)).Output-join'').Trim()-split'\s+';if($parents.Count-ne2-or$parents[0]-ne$primaryCommit-or$parents[1]-ne$primaryParent){throw 'immutable primary parent boundary mismatch'}
    $currentPrimary=Read-Json $primaryContractPath;$currentAllow=@($currentPrimary.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
    $blobContract=(@((Invoke-Git @('show',"$primaryCommit`:$primaryContractRelative")).Output)-join"`n")|ConvertFrom-Json;$blobAllow=@($blobContract.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
    $evidenceAllow=@((Invoke-Git @('show',"$primaryCommit`:$primaryAllowlistRelative")).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''})
    $completion=(@((Invoke-Git @('show',"$primaryCommit`:$primaryCompletionRelative")).Output)-join"`n")|ConvertFrom-Json;$completionAllow=@($completion.commitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
    $committed=@((Invoke-Git @('diff-tree','--no-renames','--no-commit-id','--name-only','-r',$primaryCommit)).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''})
    Compare-Set $currentAllow $blobAllow 'current/primary contract allowlist';Compare-Set $evidenceAllow $blobAllow 'primary evidence/contract allowlist';Compare-Set $completionAllow $blobAllow 'primary completion/contract allowlist';Compare-Set $committed $blobAllow 'primary commit/contract allowlist'
    if($committed.Count-ne[int]$RepairContract.primaryTask.expectedCommitPaths){throw 'immutable primary path count mismatch'}
}
function Remove-SafeScratch([string]$Path){
    if(-not(Test-Path -LiteralPath $Path)){return};$root=[System.IO.Path]::GetFullPath($cacheRoot).TrimEnd('\')+[System.IO.Path]::DirectorySeparatorChar;$full=[System.IO.Path]::GetFullPath($Path)
    $leaf=Split-Path -Leaf $full;$allowedLeaf=$leaf.StartsWith('autocrlf-')-or$leaf.StartsWith('verify-')
    if(-not$full.StartsWith($root,[StringComparison]::OrdinalIgnoreCase)-or-not$allowedLeaf){throw "unsafe scratch cleanup refused: $full"};Remove-Item -LiteralPath $full -Recurse -Force
}
function Test-AutocrlfFixture{
    [void](New-Item -ItemType Directory -Path $cacheRoot -Force);$scratch=Join-Path $cacheRoot "autocrlf-$PID-$([Guid]::NewGuid().ToString('N'))";$source=Join-Path $scratch 'source';$clone=Join-Path $scratch 'clone';[void](New-Item -ItemType Directory -Path $source -Force)
    try{
        foreach($name in @('alpha.txt','beta.json','gamma.md')){Write-Utf8 (Join-Path $source $name) "line-one`nline-two`n"}
        [void](Invoke-GitAt $source @('init','--quiet'));[void](Invoke-GitAt $source @('config','core.autocrlf','false'));[void](Invoke-GitAt $source @('config','user.name','P7 Portability Fixture'));[void](Invoke-GitAt $source @('config','user.email','p7-fixture.invalid'));[void](Invoke-GitAt $source @('add','--','alpha.txt','beta.json','gamma.md'));[void](Invoke-GitAt $source @('commit','--quiet','-m','fixture'))
        $old=$ErrorActionPreference;try{$ErrorActionPreference='Continue';$cloneOutput=@(& git -c core.autocrlf=true clone --quiet --no-local $source $clone 2>&1);$cloneCode=$LASTEXITCODE}finally{$ErrorActionPreference=$old};if($cloneCode-ne0){throw "autocrlf fixture clone failed: $((@($cloneOutput)|Select-Object -Last 3)-join' | ')"}
        [void](Invoke-GitAt $clone @('config','core.autocrlf','true'))
        [void](Invoke-GitAt $clone @('checkout-index','--force','--','alpha.txt','beta.json','gamma.md'))
        foreach($name in @('alpha.txt','beta.json','gamma.md')){[System.IO.File]::SetLastWriteTimeUtc((Join-Path $clone $name),[DateTime]::UtcNow.AddSeconds(5))}
        $forced=@((Invoke-GitAt $clone @('-c','core.autocrlf=false','status','--porcelain=v1','--untracked-files=no')).Output);$normal=@((Invoke-GitAt $clone @('status','--porcelain=v1','--untracked-files=no')).Output)
        $crlfFiles=0;$canonicalMatches=0;$rawDiffers=0
        foreach($name in @('alpha.txt','beta.json','gamma.md')){$sourceInfo=Get-CanonicalTextInfo(Join-Path $source $name);$cloneInfo=Get-CanonicalTextInfo(Join-Path $clone $name);if($cloneInfo.crlfCount-gt0){$crlfFiles++};if($cloneInfo.canonicalSha256-eq$sourceInfo.canonicalSha256){$canonicalMatches++};if($cloneInfo.rawSha256-ne$sourceInfo.rawSha256){$rawDiffers++}}
        if($normal.Count-ne0-or$forced.Count-lt1-or$crlfFiles-lt1-or$canonicalMatches-ne3-or$rawDiffers-lt1){throw "autocrlf=true fixture failed: normal=$($normal.Count) forced=$($forced.Count) crlf=$crlfFiles canonical=$canonicalMatches raw_diff=$rawDiffers"}
        return [ordered]@{coreAutocrlf='true';trackedTextFiles=3;normalStatusRows=$normal.Count;forcedFalseStatusRows=$forced.Count;observedCrlfFiles=$crlfFiles;canonicalHashMatches=$canonicalMatches;rawHashDifferences=$rawDiffers;state='PASS'}
    }finally{Remove-SafeScratch $scratch}
}

$contract=Read-Json $contractPath
if([int]$contract.schemaVersion-ne1-or[string]$contract.taskId-ne'P7.1-003-R1'-or[string]$contract.branch-ne$expectedBranch-or[string]$contract.expectedParent-ne$primaryCommit-or-not[bool]$contract.commitBoundary.finalized){throw 'R1 contract identity or finalization mismatch'}
$allowlist=@($contract.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)});Compare-Set $allowlist $expectedRepairAllowlist 'contract/hard-coded R1 allowlist';Compare-Set @($contract.commitBoundary.requiredGovernancePaths|ForEach-Object{Normalize-Path([string]$_)}) $expectedRequiredGovernance 'contract/hard-coded required governance';Compare-Set @($contract.qualityGates|ForEach-Object{[string]$_}) $expectedQualityGates 'contract/hard-coded quality gates'
foreach($field in @('rutgersRequests','databaseMutations','packageBuilds','vultrMutations','releasePublications','productionMutations')){if([int]$contract.negativeSideEffects.$field-ne0){throw "R1 negative side effect must be zero: $field"}}
$head=((Invoke-Git @('rev-parse','HEAD')).Output-join'').Trim()
if($DeriveAllowlist){if($head-ne$primaryCommit){throw 'R1 allowlist derivation is only valid at its immutable primary parent'};@(Get-DerivedAllowlist $contract)|ForEach-Object{Write-Output $_};exit 0}
if($head-eq$primaryCommit){Compare-Set (Get-DerivedAllowlist $contract) $allowlist 'derived/contract R1 allowlist'}else{$parents=((Invoke-Git @('rev-list','--parents','-n','1','HEAD')).Output-join'').Trim()-split'\s+';if($parents.Count-ne2-or$parents[0]-ne$head-or$parents[1]-ne$primaryCommit){throw 'R1 HEAD is not the direct single-parent child of the immutable primary'};$committed=@((Invoke-Git @('diff-tree','--no-renames','--no-commit-id','--name-only','-r','HEAD')).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''});Compare-Set $committed $allowlist 'R1 commit/contract allowlist'}
Assert-PrimaryBoundary $contract

$powershellExe=Join-Path $PSHOME 'powershell.exe';$primaryOutput=@(& $powershellExe -NoProfile -ExecutionPolicy Bypass -File $primaryBuilderPath -VerifyOnly 2>&1);$primaryCode=$LASTEXITCODE;if($primaryCode-ne0){throw "primary P7.1-003 builder VerifyOnly failed: $((@($primaryOutput)|Select-Object -Last 8)-join' | ')"}
$fixture=Test-AutocrlfFixture
$cargoInfo=Get-CanonicalTextInfo(Join-Path $repo 'Cargo.lock');$npmInfo=Get-CanonicalTextInfo(Join-Path $repo 'frontend\package-lock.json')

$outputRoot=if($VerifyOnly){Join-Path $cacheRoot "verify-$PID-$([Guid]::NewGuid().ToString('N'))"}else{$p7}
$outputEvidence=if($VerifyOnly){Join-Path $outputRoot 'evidence'}else{$canonicalEvidence};$outputRecords=if($VerifyOnly){Join-Path $outputRoot 'records'}else{$canonicalRecords}
try{
    [void](New-Item -ItemType Directory -Path $outputEvidence -Force);[void](New-Item -ItemType Directory -Path $outputRecords -Force)
    Write-Utf8 (Join-Path $outputEvidence 'commit-allowlist.txt') (($allowlist-join"`n")+"`n")
    $report=[ordered]@{schemaVersion=1;taskId='P7.1-003-R1';state='PASS';primary=[ordered]@{commit=$primaryCommit;parent=$primaryParent;commitPaths=73;identitySources=4};repair=[ordered]@{expectedParent=$primaryCommit;allowlistPaths=10;topology='DIRECT_SINGLE_PARENT_FAST_FORWARD_CHILD'};gitStatusPolicy=[ordered]@{usesEffectiveCheckoutConfiguration=$true;forcedCoreAutocrlfOverrideAllowed=$false;runtimeCoreAutocrlfPublished=$false};canonicalTextPolicy=[ordered]@{kind='STRICT_UTF8_CANONICAL_LF_SHA256';bomAllowed=$false;nulAllowed=$false;loneCrAllowed=$false;crlfNormalizedToLf=$true;finalNewlineRequired=$true;protectedBaselineStillRawBytes=$true};isolatedAutocrlfClone=$fixture;currentLocks=[ordered]@{cargoCanonicalSha256=$cargoInfo.canonicalSha256;frontendNpmCanonicalSha256=$npmInfo.canonicalSha256;rawWorktreeHashesPublished=$false};primaryBuilderVerifyOnly='PASS';qualityGates=@($expectedQualityGates|ForEach-Object{[ordered]@{id=$_;builderDisposition=[string]$gateDispositions[$_]}});rawCommandOutputPublished=$false;negativeSideEffects=$contract.negativeSideEffects}
    Write-Json (Join-Path $outputEvidence 'checkout-portability.json') $report
    $completionMd=@'
# P7.1-003-R1 completion record

- Task: `P7.1-003-R1`
- State: `P7_1_003_R1_PASS_COMMIT_ELIGIBLE`
- Immutable primary: `c2af7184aeec06704997f266530535003358c278`
- Topology: direct single-parent fast-forward repair child
- Repair allowlist paths: `10`
- Git status: effective checkout configuration; no forced `core.autocrlf`
- Text evidence hash: strict UTF-8 canonical LF SHA-256
- Protected baseline hash: unchanged raw-byte policy
- Isolated Windows `core.autocrlf=true` clone self-test: `PASS`
- Primary P7.1-003 builder VerifyOnly: `PASS`

No repository-wide line-ending policy was added and no historical file was renormalized. Product behavior, dependencies, Rutgers requests, database mutations, installable package builds, Vultr mutations, release publications and production mutations were unchanged or zero.

`P7.1-004` remains blocked until the repair has passed PreCommit, its independent commit, PostCommit, ordinary push, shared-worktree PostPush, and a new remote clone that emits `P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY`.
'@
    $completionMdPath=Join-Path $outputRecords 'p7-1-003-r1-completion.md';Write-Utf8 $completionMdPath ($completionMd.TrimStart()+"`n")
    $reportInfo=Get-CanonicalTextInfo(Join-Path $outputEvidence 'checkout-portability.json');$allowInfo=Get-CanonicalTextInfo(Join-Path $outputEvidence 'commit-allowlist.txt');$mdInfo=Get-CanonicalTextInfo $completionMdPath
    $completion=[ordered]@{schemaVersion=1;recordId='P7-1-003-R1-COMPLETION-2026-07-13-001';taskId='P7.1-003-R1';state='P7_1_003_R1_PASS_COMMIT_ELIGIBLE';branch=$expectedBranch;expectedParent=$primaryCommit;primaryParent=$primaryParent;commitAllowlist=$allowlist;repairAllowlistPaths=10;primaryCommitPaths=73;topology='DIRECT_SINGLE_PARENT_FAST_FORWARD_CHILD';effectiveCheckoutGitStatus=$true;canonicalTextHashPolicy='STRICT_UTF8_CANONICAL_LF_SHA256';isolatedAutocrlfCloneState='PASS';protectedWorktreeValidationProfiles=@('EXACT_PRESERVED_167','CLEAN_CHECKOUT_0');evidenceSha256=[ordered]@{'commit-allowlist.txt'=$allowInfo.canonicalSha256;'checkout-portability.json'=$reportInfo.canonicalSha256};completionMarkdownSha256=$mdInfo.canonicalSha256;negativeSideEffects=$contract.negativeSideEffects;actualCommitExcludedToAvoidSelfReference=$true;commitEligible=$true;nextTask='P7.1-004';nextTaskBlockedUntil='P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY'}
    Write-Json (Join-Path $outputRecords 'p7-1-003-r1-completion.json') $completion
    if($VerifyOnly){
        foreach($name in @('commit-allowlist.txt','checkout-portability.json')){$actual=Join-Path $canonicalEvidence $name;$expected=Join-Path $outputEvidence $name;if(-not(Test-Path -LiteralPath $actual -PathType Leaf)-or(Get-CanonicalTextInfo $actual).canonicalSha256-ne(Get-CanonicalTextInfo $expected).canonicalSha256){throw "canonical R1 evidence drift: $name"}}
        foreach($name in @('p7-1-003-r1-completion.md','p7-1-003-r1-completion.json')){$actual=Join-Path $canonicalRecords $name;$expected=Join-Path $outputRecords $name;if(-not(Test-Path -LiteralPath $actual -PathType Leaf)-or(Get-CanonicalTextInfo $actual).canonicalSha256-ne(Get-CanonicalTextInfo $expected).canonicalSha256){throw "canonical R1 completion drift: $name"}}
    }
}finally{if($VerifyOnly-and(Test-Path -LiteralPath $outputRoot)){Remove-SafeScratch $outputRoot}}

Write-Output $(if($VerifyOnly){'p7_1_003_r1_evidence=VERIFIED'}else{'p7_1_003_r1_evidence=WRITTEN'})
Write-Output "repair_allowlist_paths=$($allowlist.Count)"
Write-Output "primary_commit_paths=73"
Write-Output "fixture_normal_status_rows=$($fixture.normalStatusRows)"
Write-Output "fixture_forced_false_status_rows=$($fixture.forcedFalseStatusRows)"
Write-Output "fixture_observed_crlf_files=$($fixture.observedCrlfFiles)"
