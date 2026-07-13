[CmdletBinding()]
param(
    [ValidateSet('PreCommit','PostCommit','PostPush')]
    [string]$Mode='PreCommit',
    [switch]$RemoteCleanReplay
)

$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest

$repo=(Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath=Join-Path $p7 '03r1a-p7-1-003-r1-windows-checkout-portability-repair.json'
$baselinePath=Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$builderPath=Join-Path $p7 'tools\build-p7-1-003-r1-evidence.ps1'
$primaryContractPath=Join-Path $p7 '03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$completionPath=Join-Path $p7 'records\p7-1-003-r1-completion.json'
$primaryCommit='c2af7184aeec06704997f266530535003358c278'
$primaryParent='1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3'
$expectedBranch='codex/p7-implementation'
$expectedRemoteUrl='https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner'
$primaryContractRelative='project-governance/current/p7/03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$primaryAllowlistRelative='project-governance/current/p7/evidence/p7-1-003/commit-allowlist.txt'
$primaryCompletionRelative='project-governance/current/p7/records/p7-1-003-completion.json'
$failures=[System.Collections.Generic.List[string]]::new()
$script:Contract=$null
$script:Allowlist=@()
$script:PrimaryAllowlist=@()
$script:ProtectedProfile='UNSET'
$script:ProtectedRows=0
$script:ObservedCrlfFiles=0
$script:ForcedFalseStatusRows=0
$script:RawBlobMismatchFiles=0
$script:EffectiveCoreAutocrlf='unset'
$script:FreshCloneProof=$false
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
$expectedGateDispositions=[ordered]@{
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

function Add-Failure([string]$Message){$failures.Add($Message)}
function Assert-True([bool]$Condition,[string]$Message){if(-not$Condition){Add-Failure $Message}}
function Normalize-Path([string]$Value){return $Value.Replace('\','/')}
function Get-RawSha256([string]$Path){return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash}
function Get-CanonicalTextSha256([string]$Path){
    $bytes=[System.IO.File]::ReadAllBytes($Path);$utf8=[System.Text.UTF8Encoding]::new($false,$true);$text=$utf8.GetString($bytes)
    if($text.Length-gt0-and$text[0]-eq[char]0xFEFF){throw "UTF-8 BOM is forbidden in canonical text: $Path"};if($text.Contains([char]0)){throw "NUL is forbidden in canonical text: $Path"};if($text.Replace("`r`n",'').Contains("`r")){throw "lone CR is forbidden in canonical text: $Path"}
    $canonical=$text.Replace("`r`n","`n");if(-not$canonical.EndsWith("`n")){throw "canonical text must end with LF: $Path"};$hasher=[System.Security.Cryptography.SHA256]::Create();try{$hash=$hasher.ComputeHash([System.Text.UTF8Encoding]::new($false).GetBytes($canonical))}finally{$hasher.Dispose()};return ([System.BitConverter]::ToString($hash)).Replace('-','')
}
function Read-Json([string]$Path){if(-not(Test-Path -LiteralPath $Path -PathType Leaf)){Add-Failure 'required JSON is missing';return $null};try{return Get-Content -LiteralPath $Path -Raw -Encoding utf8|ConvertFrom-Json}catch{Add-Failure "invalid JSON: $($_.Exception.Message)";return $null}}
function Invoke-Git([string[]]$Arguments,[bool]$AllowFailure=$false){
    $old=$ErrorActionPreference;try{$ErrorActionPreference='Continue';$output=@(& git -C $repo @Arguments 2>$null);$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old};if(-not$AllowFailure-and$code-ne0){throw "git $($Arguments-join' ') failed with exit $code"};return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Assert-SameSet([string[]]$Actual,[string[]]$Expected,[string]$Label,[bool]$Redact=$false){
    $a=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$e=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($value in @($Actual)){Assert-True($a.Add([string]$value))"$Label duplicate actual"};foreach($value in @($Expected)){Assert-True($e.Add([string]$value))"$Label duplicate expected"};$missing=@($e|Where-Object{-not$a.Contains([string]$_)});$extra=@($a|Where-Object{-not$e.Contains([string]$_)});$detail=if($Redact){"missing_count=$($missing.Count) extra_count=$($extra.Count)"}else{"missing=[$($missing-join',')] extra=[$($extra-join',')]"};Assert-True($missing.Count-eq0-and$extra.Count-eq0)"$Label mismatch: $detail"
}
function Test-SafeRelativePath([string]$Path){if([string]::IsNullOrWhiteSpace($Path)-or[System.IO.Path]::IsPathRooted($Path)-or$Path.Contains('\')-or$Path-match'(^|/)\.\.(/|$)'-or$Path.Contains([char]0)){return $false};return $true}
function Test-DeniedTaskPath([string]$Path){return $Path-match'(?i)(^|/)\.secrets(/|$)'-or$Path-match'(?i)(^|/)chat-log-[^/]*\.md$'-or$Path-match'(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)'-or$Path-match'(?i)^project-governance/current/p1/'-or$Path-match'(?i)(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)'-or$Path-match'(?i)(\.sqlite3?|\.db|-wal|-shm)$'}
function Get-IndexPaths{return @((Invoke-Git @('diff','--cached','--no-renames','--name-only','--diff-filter=ACDMRTUXB')).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''})}
function Get-CommitPaths([string]$Revision){return @((Invoke-Git @('diff-tree','--no-renames','--no-commit-id','--name-only','-r',$Revision)).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''})}
function Get-LiveRemoteHead{
    $live=Invoke-Git @('ls-remote','--heads','origin',"refs/heads/$expectedBranch") $true;if($live.ExitCode-ne0-or@($live.Output).Count-ne1){return ''};return (([string]$live.Output[0])-split'\s+')[0]
}
function Test-Contract{
    $contract=Read-Json $contractPath;if($null-eq$contract){return};$script:Contract=$contract
    Assert-True([int]$contract.schemaVersion-eq1-and[string]$contract.taskId-eq'P7.1-003-R1'-and[string]$contract.state-eq'REPAIR_CONTRACT')'R1 contract identity mismatch'
    Assert-True([string]$contract.branch-eq$expectedBranch-and[string]$contract.expectedParent-eq$primaryCommit)'R1 Git boundary mismatch'
    Assert-True([string]$contract.primaryTask.expectedParent-eq$primaryParent-and[string]$contract.primaryTask.primaryCommit-eq$primaryCommit-and[int]$contract.primaryTask.expectedCommitPaths-eq73)'primary task boundary mismatch'
    Assert-True([string]$contract.scope.kind-eq'VALIDATION_PORTABILITY_ONLY'-and-not[bool]$contract.scope.repositoryWideGitattributesAllowed)'R1 scope mismatch'
    foreach($field in @('productBusinessLogicAllowed','typedDomainOrApiAllowed','runtimeBehaviorAllowed','formalUiImplementationAllowed','dependencyChangeAllowed','packageBuildAllowed','realRutgersRequestsAllowed','vultrMutationAllowed','releaseOrProductionAllowed')){Assert-True($contract.scope.$field-is[bool]-and-not$contract.scope.$field)"R1 scope must deny $field"}
    Assert-True([string]$contract.textHashPolicy.kind-eq'STRICT_UTF8_CANONICAL_LF_SHA256'-and[bool]$contract.textHashPolicy.utf8Required-and-not[bool]$contract.textHashPolicy.bomAllowed-and-not[bool]$contract.textHashPolicy.nulAllowed-and-not[bool]$contract.textHashPolicy.loneCrAllowed-and[bool]$contract.textHashPolicy.crlfNormalizedToLf-and[bool]$contract.textHashPolicy.finalNewlineRequired-and[bool]$contract.textHashPolicy.protectedBaselineStillRawBytes)'canonical text policy mismatch'
    Assert-True([bool]$contract.checkoutPolicy.gitStatusUsesEffectiveConfiguration-and-not[bool]$contract.checkoutPolicy.forcedCoreAutocrlfOverrideAllowed-and[string]$contract.checkoutPolicy.requiredCleanReplayCoreAutocrlf-eq'true'-and[string]$contract.checkoutPolicy.requiredCleanReplayProfile-eq'CLEAN_CHECKOUT_0'-and[int]$contract.checkoutPolicy.requiredObservedCrlfFilesMinimum-eq1-and[int]$contract.checkoutPolicy.requiredRawBlobMismatchFilesMinimum-eq1)'checkout policy mismatch'
    Assert-True([string]$contract.checkoutPolicy.requiredRemoteUrl-eq$expectedRemoteUrl-and[int]$contract.checkoutPolicy.freshCloneHeadReflogEntries-eq1)'remote clean replay identity mismatch'
    $allow=@($contract.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)});$script:Allowlist=$allow;Assert-True([bool]$contract.commitBoundary.finalized-and[string]$contract.commitBoundary.topology-eq'DIRECT_SINGLE_PARENT_FAST_FORWARD_CHILD')'R1 commit boundary mismatch';Assert-SameSet $allow $expectedRepairAllowlist 'contract/hard-coded R1 allowlist';Assert-SameSet @($contract.commitBoundary.requiredGovernancePaths|ForEach-Object{Normalize-Path([string]$_)}) $expectedRequiredGovernance 'contract/hard-coded required governance';Assert-SameSet @($contract.qualityGates|ForEach-Object{[string]$_}) $expectedQualityGates 'contract/hard-coded quality gates'
    foreach($path in $allow){Assert-True(Test-SafeRelativePath $path)'unsafe R1 allowlist path';Assert-True(-not(Test-DeniedTaskPath $path))'denied R1 allowlist path'}
    foreach($path in @($contract.commitBoundary.requiredGovernancePaths)){Assert-True($allow-ccontains[string]$path)'required R1 governance path absent from allowlist'}
    Assert-True(-not[bool]$contract.git.forcePush-and[bool]$contract.git.independentRepairCommit-and[bool]$contract.git.automaticPushAfterPass)'R1 Git policy mismatch'
    foreach($field in @('rutgersRequests','databaseMutations','packageBuilds','vultrMutations','releasePublications','productionMutations')){Assert-True([int]$contract.negativeSideEffects.$field-eq0)"R1 negative side effect must be zero: $field"}
    Assert-True([string]$contract.nextTask-eq'P7.1-004'-and[string]$contract.nextTaskBlockedUntil-eq'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY')'R1 next-task gate mismatch'
}
function Test-PrimaryBoundary{
    $exists=Invoke-Git @('cat-file','-e',"$primaryCommit^{commit}") $true;Assert-True($exists.ExitCode-eq0)'immutable primary commit unavailable';if($exists.ExitCode-ne0){return}
    $parents=((Invoke-Git @('rev-list','--parents','-n','1',$primaryCommit)).Output-join'').Trim()-split'\s+';Assert-True($parents.Count-eq2-and$parents[0]-eq$primaryCommit-and$parents[1]-eq$primaryParent)'immutable primary parent mismatch'
    $current=Read-Json $primaryContractPath;if($null-eq$current){return};$currentAllow=@($current.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
    $blobContract=(@((Invoke-Git @('show',"$primaryCommit`:$primaryContractRelative")).Output)-join"`n")|ConvertFrom-Json;$blobAllow=@($blobContract.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)})
    $evidenceAllow=@((Invoke-Git @('show',"$primaryCommit`:$primaryAllowlistRelative")).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_-ne''});$completion=(@((Invoke-Git @('show',"$primaryCommit`:$primaryCompletionRelative")).Output)-join"`n")|ConvertFrom-Json;$completionAllow=@($completion.commitAllowlist|ForEach-Object{Normalize-Path([string]$_)});$committed=Get-CommitPaths $primaryCommit
    Assert-SameSet $currentAllow $blobAllow 'current/primary contract allowlist';Assert-SameSet $evidenceAllow $blobAllow 'primary evidence/contract allowlist';Assert-SameSet $completionAllow $blobAllow 'primary completion/contract allowlist';Assert-SameSet $committed $blobAllow 'primary commit/contract allowlist';Assert-True($committed.Count-eq73)'immutable primary path count mismatch';$script:PrimaryAllowlist=$blobAllow
}
function Test-ProtectedWorktree{
    if(-not(Test-Path -LiteralPath $baselinePath -PathType Leaf)){Add-Failure 'protected baseline missing';return};$baseline=@(Import-Csv -LiteralPath $baselinePath -Delimiter "`t");Assert-True($baseline.Count-eq167)'protected baseline row count mismatch'
    $allow=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($path in $script:Allowlist){[void]$allow.Add($path)};$actual=@{}
    foreach($raw in @((Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output)){$line=[string]$raw;if($line.Length-lt4){continue};$code=$line.Substring(0,2);$path=Normalize-Path $line.Substring(3);if($allow.Contains($path)){continue};if($code[0]-ne' '-and$code-ne'??'){Add-Failure 'foreign path is staged'};$status=if($code-eq'??'){'UNTRACKED'}elseif($code[1]-eq'D'){'WORKTREE_D'}else{"WORKTREE_$($code[1])"};$actual[$path]=$status}
    $script:ProtectedRows=$actual.Count
    if($actual.Count-eq0){$script:ProtectedProfile='CLEAN_CHECKOUT_0';Assert-True((Invoke-Git @('check-ignore','-q','.secrets/') $true).ExitCode-eq0)'private root is not ignored';Assert-True(@((Invoke-Git @('ls-files','--','.secrets')).Output|Where-Object{$_-ne''}).Count-eq0)'private root has tracked paths';return}
    $script:ProtectedProfile='EXACT_PRESERVED_167';Assert-SameSet @($actual.Keys) @($baseline.path) 'protected path set' $true;$failCount=0
    foreach($row in $baseline){$path=[string]$row.path;if(-not$actual.ContainsKey($path)){continue};if([string]$actual[$path]-ne[string]$row.git_status){$failCount++;continue};if(-not[string]::IsNullOrWhiteSpace([string]$row.head_blob_oid)){$blob=((Invoke-Git @('rev-parse',"HEAD:$path") $true).Output-join'').Trim();if($blob-ne[string]$row.head_blob_oid){$failCount++}};$absolute=Join-Path $repo $path.Replace('/','\');if([string]$row.git_status-eq'WORKTREE_D'){if(Test-Path -LiteralPath $absolute){$failCount++};continue};if(-not(Test-Path -LiteralPath $absolute -PathType Leaf)){$failCount++;continue};if((Get-Item -LiteralPath $absolute).Length-ne[int64]$row.size_bytes){$failCount++};if([string]$row.worktree_sha256-ne'NOT_COMPUTED'){if((Get-RawSha256 $absolute)-ne[string]$row.worktree_sha256){$failCount++}}elseif([string]$row.content_policy-ne'OPAQUE_PROTECTED_NO_READ'){$failCount++}}
    Assert-True($failCount-eq0)"protected baseline validation failures: $failCount";Assert-True((Invoke-Git @('check-ignore','-q','.secrets/') $true).ExitCode-eq0)'private root is not ignored';Assert-True(@((Invoke-Git @('ls-files','--','.secrets')).Output|Where-Object{$_-ne''}).Count-eq0)'private root has tracked paths'
}
function Test-Evidence{
    if($null-eq$script:Contract){return};$allowPath=Join-Path $repo ([string]$script:Contract.evidence.commitAllowlist).Replace('/','\');$reportPath=Join-Path $repo ([string]$script:Contract.evidence.checkoutPortability).Replace('/','\');$recorded=if(Test-Path $allowPath){@(Get-Content -LiteralPath $allowPath -Encoding utf8|Where-Object{$_-ne''})}else{@()};Assert-SameSet $recorded $script:Allowlist 'R1 evidence allowlist'
    $report=Read-Json $reportPath;if($null-ne$report){Assert-True([string]$report.taskId-eq'P7.1-003-R1'-and[string]$report.state-eq'PASS'-and[int]$report.primary.commitPaths-eq73-and[int]$report.primary.identitySources-eq4-and[int]$report.repair.allowlistPaths-eq10)'R1 evidence identity mismatch';Assert-True([bool]$report.gitStatusPolicy.usesEffectiveCheckoutConfiguration-and-not[bool]$report.gitStatusPolicy.forcedCoreAutocrlfOverrideAllowed-and-not[bool]$report.gitStatusPolicy.runtimeCoreAutocrlfPublished)'R1 status evidence mismatch';Assert-True([string]$report.canonicalTextPolicy.kind-eq'STRICT_UTF8_CANONICAL_LF_SHA256'-and[bool]$report.canonicalTextPolicy.protectedBaselineStillRawBytes)'R1 canonical evidence mismatch';Assert-True([string]$report.isolatedAutocrlfClone.state-eq'PASS'-and[int]$report.isolatedAutocrlfClone.normalStatusRows-eq0-and[int]$report.isolatedAutocrlfClone.forcedFalseStatusRows-ge1-and[int]$report.isolatedAutocrlfClone.observedCrlfFiles-ge1-and[int]$report.isolatedAutocrlfClone.canonicalHashMatches-eq3)'R1 clone fixture evidence mismatch';Assert-True([string]$report.primaryBuilderVerifyOnly-eq'PASS'-and-not[bool]$report.rawCommandOutputPublished)'R1 primary replay evidence mismatch';Assert-True([string]$report.currentLocks.cargoCanonicalSha256-eq(Get-CanonicalTextSha256(Join-Path $repo 'Cargo.lock'))-and[string]$report.currentLocks.frontendNpmCanonicalSha256-eq(Get-CanonicalTextSha256(Join-Path $repo 'frontend\package-lock.json')))'R1 lock canonical hash mismatch';$reportGateIds=@($report.qualityGates|ForEach-Object{[string]$_.id});Assert-SameSet $reportGateIds $expectedQualityGates 'R1 report quality gates';foreach($gate in @($report.qualityGates)){Assert-True([string]$gate.builderDisposition-eq[string]$expectedGateDispositions[[string]$gate.id])"R1 gate disposition mismatch: $($gate.id)"};foreach($field in @('rutgersRequests','databaseMutations','packageBuilds','vultrMutations','releasePublications','productionMutations')){Assert-True([int]$report.negativeSideEffects.$field-eq0)"R1 evidence negative side effect must be zero: $field"}}
    $completion=Read-Json $completionPath;if($null-ne$completion){Assert-True([string]$completion.taskId-eq'P7.1-003-R1'-and[string]$completion.state-eq'P7_1_003_R1_PASS_COMMIT_ELIGIBLE'-and[string]$completion.expectedParent-eq$primaryCommit-and[int]$completion.repairAllowlistPaths-eq10-and[int]$completion.primaryCommitPaths-eq73)'R1 completion identity mismatch';Assert-SameSet @($completion.commitAllowlist) $script:Allowlist 'R1 completion allowlist';Assert-True([string]$completion.nextTask-eq'P7.1-004'-and[string]$completion.nextTaskBlockedUntil-eq'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY')'R1 completion next-task gate mismatch';Assert-True([string]$completion.evidenceSha256.'commit-allowlist.txt'-eq(Get-CanonicalTextSha256 $allowPath)-and[string]$completion.evidenceSha256.'checkout-portability.json'-eq(Get-CanonicalTextSha256 $reportPath))'R1 completion evidence hash mismatch';$md=Join-Path $p7 'records\p7-1-003-r1-completion.md';Assert-True([string]$completion.completionMarkdownSha256-eq(Get-CanonicalTextSha256 $md))'R1 completion Markdown hash mismatch';foreach($field in @('rutgersRequests','databaseMutations','packageBuilds','vultrMutations','releasePublications','productionMutations')){Assert-True([int]$completion.negativeSideEffects.$field-eq0)"R1 completion negative side effect must be zero: $field"}}
}
function Test-PublicationText([string]$Content,[string]$Label){$patterns=@('-----BEGIN [A-Z ]*PRIVATE KEY-----','(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}','github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}','npm_[A-Za-z0-9]{20,}','sk-(proj-)?[A-Za-z0-9_-]{20,}','eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}','(?i)https?://[^/\s:@]+:[^/\s@]+@','(?i)(^|[\s"''])[A-Z]:[\\/]','(?i)/(home|Users|mnt/[a-z])/[^/\s"'']+/','(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}');foreach($pattern in $patterns){Assert-True(-not[regex]::IsMatch($Content,$pattern))"publication safety failure in $Label"}}
function Test-PublicationBlobs([string]$Source,[string[]]$Paths){foreach($path in $Paths){$spec=if($Source-eq'index'){":$path"}else{"$Source`:$path"};$exists=Invoke-Git @('cat-file','-e',$spec) $true;if($exists.ExitCode-ne0){continue};$blob=Invoke-Git @('show',$spec) $true;Assert-True($blob.ExitCode-eq0)'cannot read allowed publication blob';if($blob.ExitCode-eq0){Test-PublicationText(@($blob.Output)-join"`n")"$Source allowed blob"}}}
function Test-CleanReplayProof{
    if(-not$RemoteCleanReplay){return};Assert-True($Mode-eq'PostPush')'RemoteCleanReplay is only valid in PostPush mode';Assert-True($script:ProtectedProfile-eq'CLEAN_CHECKOUT_0')'remote replay requires clean checkout profile'
    $core=Invoke-Git @('config','--get','core.autocrlf') $true;$script:EffectiveCoreAutocrlf=if($core.ExitCode-eq0){((@($core.Output)-join'').Trim()).ToLowerInvariant()}else{'unset'};Assert-True($script:EffectiveCoreAutocrlf-eq'true')'remote replay requires effective core.autocrlf=true'
    $origin=((Invoke-Git @('remote','get-url','origin')).Output-join'').Trim().TrimEnd('/');Assert-True($origin-eq$expectedRemoteUrl)'remote replay origin identity mismatch';$reflog=@((Invoke-Git @('reflog','show','--format=%gs','HEAD')).Output);$script:FreshCloneProof=$reflog.Count-eq1-and[string]$reflog[0]-like'clone: from *';Assert-True($script:FreshCloneProof)'remote replay is not a fresh clone'
    $normalAfterGates=@((Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output);Assert-True($normalAfterGates.Count-eq0)'remote replay became dirty after validation gates'
    $safe=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($path in @($script:Allowlist+$script:PrimaryAllowlist)){[void]$safe.Add($path)};$crlf=0;$rawMismatch=0
    foreach($path in $safe){$absolute=Join-Path $repo $path.Replace('/','\');if(-not(Test-Path -LiteralPath $absolute -PathType Leaf)){continue};$bytes=[System.IO.File]::ReadAllBytes($absolute);for($i=1;$i-lt$bytes.Length;$i++){if($bytes[$i-1]-eq0x0D-and$bytes[$i]-eq0x0A){$crlf++;break}};$rawOid=((Invoke-Git @('hash-object','--no-filters','--',$path) $true).Output-join'').Trim();$blobOid=((Invoke-Git @('rev-parse',"HEAD:$path") $true).Output-join'').Trim();if($rawOid-ne''-and$blobOid-ne''-and$rawOid-ne$blobOid){$rawMismatch++}};$script:ObservedCrlfFiles=$crlf;$script:RawBlobMismatchFiles=$rawMismatch;Assert-True($crlf-ge1)'remote replay did not observe native CRLF text';Assert-True($rawMismatch-ge1)'remote replay did not prove raw worktree/blob line-ending divergence'
    $forced=@((Invoke-Git @('-c','core.autocrlf=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output);$script:ForcedFalseStatusRows=$forced.Count
}

Push-Location $repo
try{
    Test-Contract;Test-PrimaryBoundary;Test-ProtectedWorktree
    if($null-ne$script:Contract){$powershellExe=Join-Path $PSHOME 'powershell.exe';$builder=@(& $powershellExe -NoProfile -ExecutionPolicy Bypass -File $builderPath -VerifyOnly 2>&1);$builderCode=$LASTEXITCODE;Assert-True($builderCode-eq0)"R1 evidence builder VerifyOnly failed: $((@($builder)|Select-Object -Last 8)-join' | ')"}
    Test-Evidence
    $branch=((Invoke-Git @('branch','--show-current')).Output-join'').Trim();Assert-True($branch-eq$expectedBranch)'R1 branch mismatch';$head=((Invoke-Git @('rev-parse','HEAD')).Output-join'').Trim();$remoteResult=Invoke-Git @('rev-parse',"refs/remotes/origin/$expectedBranch") $true;$remoteHead=if($remoteResult.ExitCode-eq0){((@($remoteResult.Output)-join'').Trim())}else{''};$liveHead=Get-LiveRemoteHead;Assert-True($remoteResult.ExitCode-eq0)'R1 tracking ref unavailable'
    Test-PublicationBlobs $primaryCommit $script:PrimaryAllowlist
    if($Mode-eq'PreCommit'){
        Assert-True($head-eq$primaryCommit-and$remoteHead-eq$primaryCommit-and$liveHead-eq$primaryCommit)'R1 PreCommit remote boundary mismatch';Assert-SameSet (Get-IndexPaths) $script:Allowlist 'R1 staged allowlist';$unstaged=@((Invoke-Git (@('diff','--name-only','--')+$script:Allowlist)).Output|Where-Object{$_-ne''});Assert-True($unstaged.Count-eq0)'R1 task paths remain unstaged';$check=@((Invoke-Git @('diff','--cached','--check') $true).Output);Assert-True($check.Count-eq0)'R1 cached diff check failed';Test-PublicationBlobs 'index' $script:Allowlist
    }else{
        $parents=((Invoke-Git @('rev-list','--parents','-n','1','HEAD')).Output-join'').Trim()-split'\s+';Assert-True($parents.Count-eq2-and$parents[0]-eq$head-and$parents[1]-eq$primaryCommit)'R1 commit is not the direct single-parent primary child';Assert-SameSet (Get-CommitPaths 'HEAD') $script:Allowlist 'R1 committed allowlist';$index=@(Get-IndexPaths);Assert-True($index.Count-eq0)'R1 index is not empty';$unstaged=@((Invoke-Git (@('diff','--name-only','--')+$script:Allowlist)).Output|Where-Object{$_-ne''});Assert-True($unstaged.Count-eq0)'R1 committed task paths are dirty';$check=@((Invoke-Git @('diff-tree','--check','HEAD^','HEAD') $true).Output);Assert-True($check.Count-eq0)'R1 committed diff check failed';Test-PublicationBlobs 'HEAD' $script:Allowlist;$message=((Invoke-Git @('log','-1','--format=%B','HEAD')).Output-join"`n");Test-PublicationText $message 'R1 commit message'
        if($Mode-eq'PostCommit'){Assert-True($remoteHead-eq$primaryCommit-and$liveHead-eq$primaryCommit)'R1 remote advanced before PostCommit validation'}else{Assert-True($remoteHead-eq$head-and$liveHead-eq$head)'R1 PostPush remote boundary mismatch';$upstream=Invoke-Git @('rev-parse','--abbrev-ref','--symbolic-full-name','@{upstream}') $true;Assert-True($upstream.ExitCode-eq0-and((@($upstream.Output)-join'').Trim()-eq"origin/$expectedBranch"))'R1 upstream mismatch'}
    }
    Assert-True(@((Invoke-Git @('ls-files','--','.gitattributes')).Output|Where-Object{$_-ne''}).Count-eq0)'R1 must not add repository-wide gitattributes';Test-CleanReplayProof
}finally{Pop-Location}

if($RemoteCleanReplay-and-not($Mode-eq'PostPush'-and$script:ProtectedProfile-eq'CLEAN_CHECKOUT_0'-and$script:EffectiveCoreAutocrlf-eq'true'-and$script:ObservedCrlfFiles-ge1-and$script:RawBlobMismatchFiles-ge1-and$script:FreshCloneProof)){Add-Failure 'terminal remote clean replay conditions are incomplete'}
if($failures.Count-gt0){Write-Output 'p7_1_003_r1_gate=FAIL';$failures|ForEach-Object{Write-Output "failure=$_"};exit 1}
$state=if($RemoteCleanReplay){'P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY'}elseif($Mode-eq'PreCommit'){'P7_1_003_R1_PASS_PRE_COMMIT'}elseif($Mode-eq'PostCommit'){'P7_1_003_R1_PASS_POST_COMMIT'}else{'P7_1_003_R1_PASS_POST_PUSH'}
Write-Output "validation_mode=$Mode"
Write-Output "validation_state=$state"
Write-Output 'active_task=P7.1-003-R1'
Write-Output "repair_allowlist_paths=$($script:Allowlist.Count)"
Write-Output "primary_commit_paths=$($script:PrimaryAllowlist.Count)"
Write-Output "protected_worktree_rows=$($script:ProtectedRows)"
Write-Output "protected_worktree_profile=$($script:ProtectedProfile)"
Write-Output "effective_core_autocrlf=$($script:EffectiveCoreAutocrlf)"
Write-Output "observed_crlf_files=$($script:ObservedCrlfFiles)"
Write-Output "forced_false_status_rows=$($script:ForcedFalseStatusRows)"
Write-Output "raw_blob_mismatch_files=$($script:RawBlobMismatchFiles)"
Write-Output 'rutgers_requests=0'
Write-Output 'database_mutations=0'
Write-Output 'package_builds=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_003_r1_gate=PASS'
