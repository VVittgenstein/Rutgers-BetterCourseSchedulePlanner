[CmdletBinding()]
param(
    [ValidateSet('PreCommit','PostCommit','PostPush')]
    [string]$Mode='PreCommit'
)

$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$repo=(Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath=Join-Path $p7 '03a-p7-1-003-workspace-module-graph-and-capability-build-guards.json'
$baselinePath=Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$previousCompletionPath=Join-Path $p7 'records\p7-1-002-completion.json'
$completionPath=Join-Path $p7 'records\p7-1-003-completion.json'
$builderPath=Join-Path $p7 'tools\build-p7-1-003-evidence.ps1'
$expectedParent='1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3'
$primaryCommit='c2af7184aeec06704997f266530535003358c278'
$expectedBranch='codex/p7-implementation'
$failures=[System.Collections.Generic.List[string]]::new()
$script:Contract=$null
$script:Allowlist=@()
$script:ProtectedWorktreeRows=0
$script:ProtectedWorktreeProfile='UNSET'

function Add-Failure([string]$Message){$failures.Add($Message)}
function Assert-True([bool]$Condition,[string]$Message){if(-not $Condition){Add-Failure $Message}}
function Normalize-Path([string]$Value){return $Value.Replace('\','/')}
function Get-Sha256([string]$Path){return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash}
function Get-CanonicalTextSha256([string]$Path){
    $bytes=[System.IO.File]::ReadAllBytes($Path);$utf8=[System.Text.UTF8Encoding]::new($false,$true);$text=$utf8.GetString($bytes)
    if($text.Length-gt0-and$text[0]-eq[char]0xFEFF){throw "UTF-8 BOM is forbidden in canonical text: $Path"}
    if($text.Contains([char]0)){throw "NUL is forbidden in canonical text: $Path"};if($text.Replace("`r`n",'').Contains("`r")){throw "lone CR is forbidden in canonical text: $Path"}
    $canonical=$text.Replace("`r`n","`n");if(-not$canonical.EndsWith("`n")){throw "canonical text must end with LF: $Path"}
    $hasher=[System.Security.Cryptography.SHA256]::Create();try{$hash=$hasher.ComputeHash([System.Text.UTF8Encoding]::new($false).GetBytes($canonical))}finally{$hasher.Dispose()};return([System.BitConverter]::ToString($hash)).Replace('-','')
}
function Read-Json([string]$Path){
    if(-not(Test-Path -LiteralPath $Path -PathType Leaf)){Add-Failure "missing JSON: $(Normalize-Path $Path)";return $null}
    try{return Get-Content -LiteralPath $Path -Raw -Encoding utf8|ConvertFrom-Json}catch{Add-Failure "invalid JSON: $(Normalize-Path $Path): $($_.Exception.Message)";return $null}
}
function Invoke-Git([string[]]$Arguments,[bool]$AllowFailure=$false){
    $old=$ErrorActionPreference
    try{$ErrorActionPreference='Continue';$output=@(& git -C $repo @Arguments 2>$null);$code=$LASTEXITCODE}finally{$ErrorActionPreference=$old}
    if(-not $AllowFailure -and $code -ne 0){throw "git $($Arguments -join ' ') failed with exit $code"}
    return [pscustomobject]@{Output=$output;ExitCode=$code}
}
function Assert-SameSet([string[]]$Actual,[string[]]$Expected,[string]$Label){
    $a=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);$e=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($v in @($Actual)){Assert-True($a.Add([string]$v))"$Label duplicate actual: $v"}
    foreach($v in @($Expected)){Assert-True($e.Add([string]$v))"$Label duplicate expected: $v"}
    $missing=@($e|Where-Object{-not $a.Contains([string]$_)});$extra=@($a|Where-Object{-not $e.Contains([string]$_)})
    Assert-True($missing.Count -eq 0 -and $extra.Count -eq 0)"$Label mismatch: missing=[$($missing -join ',')] extra=[$($extra -join ',')]"
}
function Test-SafeRelativePath([string]$Path){
    if([string]::IsNullOrWhiteSpace($Path)-or[System.IO.Path]::IsPathRooted($Path)-or$Path-match '(^|/)\.\.(/|$)' -or $Path-match '^[A-Za-z]:'){return $false}
    return $true
}
function Test-DeniedTaskPath([string]$Path){
    return $Path-match '(?i)(^|/)\.secrets(/|$)' -or $Path-match '(?i)(^|/)chat-log-[^/]*\.md$' -or $Path-match '(?i)^docs/(chat-log-|sessions/|p1-a-recovery/)' -or $Path-match '(?i)^project-governance/current/p1/' -or $Path-match '(?i)(^|/)(node_modules|dist|target|\.cache|\.ngagent|\.orchestrator)(/|$)' -or $Path-match '(?i)(\.sqlite3?|\.db|-wal|-shm)$'
}
function Get-IndexPaths{return @((Invoke-Git @('diff','--cached','--no-renames','--name-only','--diff-filter=ACDMRTUXB')).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_ -ne ''})}
function Get-CommitPaths([string]$Revision){return @((Invoke-Git @('diff-tree','--no-renames','--no-commit-id','--name-only','-r',$Revision)).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_ -ne ''})}

function Test-Contract{
    $c=Read-Json $contractPath;if($null -eq $c){return};$script:Contract=$c
    Assert-True([int]$c.schemaVersion -eq 1 -and [string]$c.taskId -eq 'P7.1-003')'contract identity mismatch'
    Assert-True([string]$c.branch -eq $expectedBranch -and [string]$c.expectedParent -eq $expectedParent -and [string]$c.expectedRemoteBaseline -eq $expectedParent)'contract Git boundary mismatch'
    Assert-True([string]$c.dependency.taskId -eq 'P7.1-002' -and [string]$c.dependency.requiredCommit -eq $expectedParent -and [string]$c.dependency.requiredRemoteState -eq 'P7_1_002_PASS_POST_PUSH')'contract predecessor mismatch'
    Assert-True([string]$c.scope.kind -eq 'WORKSPACE_AND_SOURCE_GRAPH_ONLY')'contract scope mismatch'
    foreach($field in @('productBusinessLogicAllowed','typedDomainOrApiAllowed','runtimeBehaviorAllowed','formalUiImplementationAllowed','packageBuildAllowed','realRutgersRequestsAllowed','vultrMutationAllowed','releaseOrProductionAllowed')){Assert-True($c.scope.$field -is [bool] -and -not $c.scope.$field)"scope must deny $field"}
    Assert-True([int]$c.rustGraph.workspaceCount -eq 1 -and [string]$c.rustGraph.resolver -eq '3' -and [int]$c.rustGraph.workspacePackageCount -eq 15 -and [int]$c.rustGraph.workspaceDefaultMemberCount -eq 15)'Rust workspace cardinality/default-member mismatch'
    Assert-SameSet @($c.rustGraph.rootManifestSections) @('workspace','workspace.package','workspace.dependencies','workspace.metadata.p7.tooling') 'root Cargo manifest sections'
    Assert-True([string]$c.rustGraph.internalPathDependencyVersion -eq '=0.0.0')'internal path dependency version policy mismatch'
    $expectedPackages=@('bcsp-contracts','bcsp-domain','bcsp-query','bcsp-rutgers-client','bcsp-operational-storage','bcsp-catalog','bcsp-open','bcsp-watch','bcsp-application','bcsp-local-user-state','bcsp-local-runtime','bcsp-public-operations','bcsp-public-runtime','bcsp-local','bcsp-server')
    Assert-SameSet @($c.rustGraph.packages|ForEach-Object{[string]$_.name}) $expectedPackages 'contract Rust packages'
    foreach($p in @($c.rustGraph.packages)){
        Assert-True (Test-SafeRelativePath ([string]$p.manifest)) "unsafe package manifest: $($p.name)"
        $manifestPath=Join-Path $repo ([string]$p.manifest).Replace('/','\')
        Assert-True(Test-Path -LiteralPath $manifestPath -PathType Leaf)"missing package manifest: $($p.manifest)"
        if(Test-Path -LiteralPath $manifestPath -PathType Leaf){foreach($line in @(Get-Content -LiteralPath $manifestPath -Encoding utf8|Where-Object{$_ -match '\bpath\s*=\s*"\.\.(?:/|\\)'})){Assert-True($line -match 'version\s*=\s*"=0\.0\.0"')"unversioned internal path dependency in $($p.manifest): $($line.Trim())"}}
    }
    Assert-SameSet @($c.rustGraph.sharedPackages) @('bcsp-contracts','bcsp-domain','bcsp-query','bcsp-rutgers-client','bcsp-operational-storage','bcsp-catalog','bcsp-open','bcsp-watch','bcsp-application') 'shared packages'
    Assert-SameSet @($c.rustGraph.localOnlyPackages) @('bcsp-local-user-state','bcsp-local-runtime') 'local packages'
    Assert-SameSet @($c.rustGraph.publicOnlyPackages) @('bcsp-public-operations','bcsp-public-runtime') 'public packages'
    Assert-True([string]$c.rustGraph.entries.local -eq 'bcsp-local' -and [string]$c.rustGraph.entries.public -eq 'bcsp-server')'entry names mismatch'
    Assert-True([string]$c.rustGraph.guard -eq 'tools/architecture/verify-rust-graph.mjs' -and [string]$c.rustGraph.guardTest -eq 'tools/architecture/verify-rust-graph.test.mjs')'Rust guard paths mismatch'
    Assert-SameSet @($c.rustGraph.publicSourceClosurePackages) @('bcsp-contracts','bcsp-domain','bcsp-query','bcsp-rutgers-client','bcsp-operational-storage','bcsp-catalog','bcsp-open','bcsp-watch','bcsp-application','bcsp-public-operations','bcsp-public-runtime','bcsp-server') 'Rust public SOURCE closure packages'
    Assert-True([int]$c.rustGraph.publicSourceRowCount -eq 18 -and [int]$c.rustGraph.publicSourceMarkerCount -eq 212)'Rust public SOURCE contract mismatch'
    Assert-True([string]$c.frontendGraph.roots.shared -eq 'frontend/src/ui/shared' -and [string]$c.frontendGraph.roots.local -eq 'frontend/src/ui/local' -and [string]$c.frontendGraph.roots.public -eq 'frontend/src/ui/public')'frontend root paths mismatch'
    Assert-True([string]$c.frontendGraph.entries.local -eq 'frontend/src/entry.local.tsx' -and [string]$c.frontendGraph.entries.public -eq 'frontend/src/entry.public.tsx')'frontend entries mismatch'
    Assert-True([string]$c.frontendGraph.guard -eq 'frontend/tools/verify-import-graph.mjs' -and [string]$c.frontendGraph.guardTest -eq 'frontend/tools/verify-import-graph.test.mjs')'frontend guard paths mismatch'
    Assert-True([int]$c.frontendGraph.publicSourceRowCount -eq 18 -and [int]$c.frontendGraph.publicSourceMarkerCount -eq 212)'frontend public SOURCE contract mismatch'
    Assert-True([int]$c.p4SourceCoverage.sourceRowCount -eq 18 -and [int]$c.p4SourceCoverage.remainingRowCount -eq 126 -and [string]$c.p4SourceCoverage.remainingRowsOwner -eq 'P7.1-013')'P4 row ownership mismatch'
    Assert-True([string]$c.p4SourceCoverage.guardInput -eq 'tools/architecture/p4-public-source-deny.json' -and [int]$c.p4SourceCoverage.snapshotSchemaVersion -eq 2 -and [int]$c.p4SourceCoverage.markerSetVersion -eq 1 -and [string]$c.p4SourceCoverage.snapshotKind -eq 'P4_PUBLIC_SOURCE_DENY_FROZEN_SNAPSHOT' -and [string]$c.p4SourceCoverage.snapshotMode -eq 'FROZEN_P7_1_003_NO_LIVE_SOURCE_DEPENDENCY' -and [string]$c.p4SourceCoverage.provenanceReference -eq 'project-governance/current/p4/05-public-capability-deny-audit.tsv' -and -not [bool]$c.p4SourceCoverage.runtimeSourceRequired)'P4 shared SOURCE frozen snapshot mismatch'
    Assert-SameSet @($c.p4SourceCoverage.requiredClosures) @('RUST_BCSP_SERVER','FRONTEND_PUBLIC_ENTRY') 'P4 required public SOURCE closures'
    $snapshot=Read-Json (Join-Path $repo ([string]$c.p4SourceCoverage.guardInput).Replace('/','\'))
    if($null-ne$snapshot){Assert-True([int]$snapshot.schemaVersion -eq 2 -and [int]$snapshot.markerSetVersion -eq 1 -and [string]$snapshot.kind -eq 'P4_PUBLIC_SOURCE_DENY_FROZEN_SNAPSHOT' -and [string]$snapshot.snapshotMode -eq 'FROZEN_P7_1_003_NO_LIVE_SOURCE_DEPENDENCY' -and -not[bool]$snapshot.runtimeSourceRequired)'P4 snapshot file identity mismatch';Assert-SameSet @($snapshot.capabilities|ForEach-Object{[string]$_.denyId}) @($c.p4SourceCoverage.denyIds) 'P4 snapshot deny IDs';foreach($row in @($snapshot.capabilities)){Assert-True([string]$row.validationId -eq ([string]$row.denyId).Replace('P4-D-','P4-Z-'))"P4 snapshot validation identity mismatch: $($row.denyId)"}}
    Assert-True([string]$c.lockArtifacts.previousCargoLockSha256 -eq '93992B1D426C91202203E06ACE3BF55D21A91510A3C4E0D6D0CFED422F410263' -and [string]$c.lockArtifacts.frontendNpmSha256 -eq '97322B86E314BE69E4601CD4EE9187E359AA9FDB7B9186F193EF1D98B5A06466' -and [bool]$c.lockArtifacts.cargoLockChangeExpected -and -not[bool]$c.lockArtifacts.frontendNpmLockChangeExpected)'lock artifact contract mismatch'
    Assert-True([string]$c.legacyDisposition.zeroConsumerScope -eq 'ACTIVE_P7_TARGET_GRAPH_ONLY' -and -not [bool]$c.legacyDisposition.repositoryWideZeroConsumerClaim -and [int]$c.legacyDisposition.requiredActiveGraphReachability -eq 0)'legacy scope mismatch'
    Assert-SameSet @($c.legacyDisposition.ownedRiskIds) @('P6-D031','P6-D034','P6-D042') 'legacy risk IDs'
    Assert-SameSet @($c.protectedWorktreeValidation.profiles) @('EXACT_PRESERVED_167','CLEAN_CHECKOUT_0') 'protected worktree validation profiles'
    Assert-True(-not[bool]$c.protectedWorktreeValidation.partialProtectedProfileAllowed -and -not[bool]$c.protectedWorktreeValidation.cleanCheckoutRequiresProtectedPaths)'protected worktree clean-profile contract mismatch'
    Assert-True($c.commitBoundary.finalized -is [bool] -and [bool]$c.commitBoundary.finalized)'contract allowlist is not finalized'
    $allow=@($c.commitBoundary.taskCommitAllowlist|ForEach-Object{Normalize-Path([string]$_)});$script:Allowlist=$allow
    Assert-True($allow.Count -gt 13)'contract allowlist is implausibly small';Assert-SameSet $allow $allow 'contract allowlist uniqueness'
    foreach($path in $allow){Assert-True(Test-SafeRelativePath $path)"unsafe allowlist path: $path";Assert-True(-not(Test-DeniedTaskPath $path))"denied allowlist path: $path"}
    foreach($path in @($c.commitBoundary.requiredGovernancePaths)){Assert-True($allow -ccontains [string]$path)"required governance path absent from allowlist: $path"}
    foreach($required in @('README.md','Cargo.toml','Cargo.lock','tools/architecture/p4-public-source-deny.json','tools/architecture/verify-rust-graph.mjs','tools/architecture/verify-rust-graph.test.mjs','frontend/package.json','frontend/tools/verify-import-graph.mjs','frontend/tools/verify-import-graph.test.mjs','frontend/src/entry.local.tsx','frontend/src/entry.public.tsx')){Assert-True($allow -ccontains $required)"required task path absent: $required"}
    Assert-True([string]$c.nextTask -eq 'P7.1-004' -and [string]$c.nextTaskBlockedUntil -eq 'P7_1_003_PASS_POST_PUSH')'next-task boundary mismatch'
}

function Test-PreviousTask{
    $previous=Read-Json $previousCompletionPath
    if($null -ne $previous){Assert-True([string]$previous.taskId -eq 'P7.1-002' -and [string]$previous.state -eq 'P7_1_002_PASS_COMMIT_ELIGIBLE')'P7.1-002 completion mismatch';Assert-True([int]$previous.protectedBaselineRows -eq 167)'previous protected count mismatch'}
    $exists=Invoke-Git @('cat-file','-e',"$expectedParent^{commit}") $true;Assert-True($exists.ExitCode -eq 0)'expected predecessor commit is unavailable'
    if($exists.ExitCode -eq 0){$paths=Get-CommitPaths $expectedParent;Assert-True($paths.Count -eq 26)'P7.1-002 committed path count mismatch'}
}
function Test-ProtectedBaseline{
    if(-not(Test-Path -LiteralPath $baselinePath -PathType Leaf)){Add-Failure 'protected baseline missing';return}
    $baseline=@(Import-Csv -LiteralPath $baselinePath -Delimiter "`t");Assert-True($baseline.Count -eq 167)"protected baseline rows: $($baseline.Count)"
    $allow=[System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal);foreach($p in $script:Allowlist){[void]$allow.Add($p)}
    $actual=@{}
    foreach($raw in @((Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output)){
        $line=[string]$raw;if($line.Length -lt 4){continue};$code=$line.Substring(0,2);$path=Normalize-Path $line.Substring(3);if($allow.Contains($path)){continue}
        if($code[0] -ne ' ' -and $code -ne '??'){Add-Failure "foreign path staged: $path ($code)"}
        $status=if($code -eq '??'){'UNTRACKED'}elseif($code[1] -eq 'D'){'WORKTREE_D'}else{"WORKTREE_$($code[1])"};$actual[$path]=$status
    }
    $script:ProtectedWorktreeRows=$actual.Count
    if($actual.Count -eq 0){
        $script:ProtectedWorktreeProfile='CLEAN_CHECKOUT_0'
        Assert-True((Invoke-Git @('check-ignore','-q','.secrets/') $true).ExitCode -eq 0)'private root is not ignored'
        Assert-True(@((Invoke-Git @('ls-files','--','.secrets')).Output|Where-Object{$_ -ne ''}).Count -eq 0)'private root has tracked paths'
        return
    }
    $script:ProtectedWorktreeProfile='EXACT_PRESERVED_167'
    Assert-SameSet @($actual.Keys) @($baseline.path) 'protected path set'
    foreach($row in $baseline){
        $path=[string]$row.path;if(-not $actual.ContainsKey($path)){continue};Assert-True([string]$actual[$path] -eq [string]$row.git_status)"protected status drift: $path"
        if(-not [string]::IsNullOrWhiteSpace([string]$row.head_blob_oid)){$blob=((Invoke-Git @('rev-parse',"HEAD:$path") $true).Output-join'').Trim();Assert-True($blob -eq [string]$row.head_blob_oid)"protected HEAD blob drift: $path"}
        $absolute=Join-Path $repo $path.Replace('/','\')
        if([string]$row.git_status -eq 'WORKTREE_D'){
            Assert-True(-not(Test-Path -LiteralPath $absolute))"protected deletion reappeared: $path"
            continue
        }
        Assert-True(Test-Path -LiteralPath $absolute -PathType Leaf)"protected file missing: $path"
        if(Test-Path -LiteralPath $absolute -PathType Leaf){Assert-True((Get-Item -LiteralPath $absolute).Length -eq [int64]$row.size_bytes)"protected size drift: $path";if([string]$row.worktree_sha256 -ne 'NOT_COMPUTED'){Assert-True((Get-Sha256 $absolute)-eq[string]$row.worktree_sha256)"protected hash drift: $path"}else{Assert-True([string]$row.content_policy -eq 'OPAQUE_PROTECTED_NO_READ')"opaque policy drift: $path"}}
    }
    Assert-True((Invoke-Git @('check-ignore','-q','.secrets/') $true).ExitCode -eq 0)'private root is not ignored'
    Assert-True(@((Invoke-Git @('ls-files','--','.secrets')).Output|Where-Object{$_ -ne ''}).Count -eq 0)'private root has tracked paths'
}
function Test-Evidence{
    if($null -eq $script:Contract){return}
    $e=$script:Contract.evidence
    $allowPath=Join-Path $repo ([string]$e.commitAllowlist).Replace('/','\');if(Test-Path $allowPath){$recorded=@(Get-Content -LiteralPath $allowPath -Encoding utf8|Where-Object{$_ -ne ''});Assert-SameSet $recorded $script:Allowlist 'evidence allowlist'}else{Add-Failure 'allowlist evidence missing'}
    $rust=Read-Json (Join-Path $repo ([string]$e.rustWorkspaceGraph).Replace('/','\'));if($null-ne$rust){Assert-True([int]$rust.schemaVersion -eq 2 -and [bool]$rust.ok -and [string]$rust.cargoResolver -eq '3' -and [int]$rust.workspaceMembers -eq 15 -and [int]$rust.workspaceDefaultMembers -eq 15)'Rust graph evidence failed';Assert-SameSet @($rust.rootManifestSections) @($script:Contract.rustGraph.rootManifestSections) 'Rust root manifest evidence';Assert-SameSet @($rust.binaryTargets) @('bcsp-local','bcsp-server') 'Rust binary targets';Assert-SameSet @($rust.publicSourceDeny.publicClosurePackages) @($script:Contract.rustGraph.publicSourceClosurePackages) 'Rust evidence public SOURCE closure';Assert-True([int]$rust.publicSourceDeny.expectedCount -eq 18 -and [int]$rust.publicSourceDeny.checkedCount -eq 18 -and [int]$rust.publicSourceDeny.markerCount -eq 212 -and [int]$rust.publicSourceDeny.scannedFileCount -ge 12 -and [int]$rust.publicSourceDeny.sourceEscapeCount -eq 0 -and [int]$rust.publicSourceDeny.violationCount -eq 0)'Rust P4 SOURCE evidence mismatch'}
    $front=Read-Json (Join-Path $repo ([string]$e.frontendImportGraph).Replace('/','\'));if($null-ne$front){Assert-True([string]$front.state -eq 'PASS')'frontend graph evidence failed';Assert-True([int]$front.publicSourceDeny.expectedCount -eq 18 -and [int]$front.publicSourceDeny.checkedCount -eq 18 -and [int]$front.publicSourceDeny.expectedMarkerCount -eq 212 -and [int]$front.publicSourceDeny.checkedMarkerCount -eq 212 -and [int]$front.publicSourceDeny.violationCount -eq 0)'frontend P4 SOURCE evidence mismatch';Assert-True([int]$front.prohibitedConstructs.count -eq 0 -and [bool]$front.repositoryContract.pass)'frontend static/source-loading contract mismatch';Assert-True(-not[bool]$front.assertions.legacySourceReachable -and [int]$front.assertions.publicToLocalEdges -eq 0 -and [int]$front.assertions.sharedToTargetEdges -eq 0)'frontend reachability evidence mismatch'}
    $quality=Read-Json (Join-Path $repo ([string]$e.qualityGates).Replace('/','\'));if($null-ne$quality){Assert-True([string]$quality.state -eq 'PASS' -and -not[bool]$quality.rawCommandOutputPublished)'quality evidence state mismatch';Assert-SameSet @($quality.gates|ForEach-Object{[string]$_.id}) @($script:Contract.qualityGates) 'quality gate IDs';foreach($g in @($quality.gates)){Assert-True([int]$g.exitCode -eq 0 -and [string]$g.normalizedResult -eq 'PASS')"failed quality gate: $($g.id)"}}
    $delta=Read-Json (Join-Path $repo ([string]$e.dependencyClosureDelta).Replace('/','\'));if($null-ne$delta){$cargoHash=Get-CanonicalTextSha256 (Join-Path $repo 'Cargo.lock');$npmHash=Get-CanonicalTextSha256 (Join-Path $repo 'frontend\package-lock.json');Assert-True([string]$delta.state -eq 'PASS' -and [int]$delta.rustAdded -eq 0 -and [int]$delta.rustRemoved -eq 0 -and [bool]$delta.cargoLockChanged -and -not[bool]$delta.npmLockChanged)'dependency closure/lock disposition changed';Assert-True([string]$delta.previousCargoLockSha256 -eq [string]$script:Contract.lockArtifacts.previousCargoLockSha256 -and [string]$delta.currentCargoLockSha256 -eq $cargoHash -and [string]$delta.npmLockSha256 -eq $npmHash)'dependency lock hash evidence mismatch'}
    $p4Path=Join-Path $repo ([string]$e.p4SourceCoverage).Replace('/','\');if(Test-Path $p4Path){$rows=@(Import-Csv -LiteralPath $p4Path -Delimiter "`t");Assert-True($rows.Count -eq 18)'P4 evidence row count mismatch';Assert-SameSet @($rows|ForEach-Object{[string]$_.deny_id}) @($script:Contract.p4SourceCoverage.denyIds) 'P4 evidence deny IDs';foreach($r in $rows){Assert-True([string]$r.validation_id -eq ([string]$r.deny_id).Replace('P4-D-','P4-Z-'))"P4 validation identity mismatch: $($r.deny_id)";Assert-True([string]$r.rust_source_status -eq 'PASS' -and [string]$r.frontend_source_status -eq 'PASS' -and [string]$r.task_status -eq 'PASS_BOTH_PUBLIC_SOURCE_CLOSURES')"P4 row failed: $($r.deny_id)"}}else{Add-Failure 'P4 SOURCE evidence missing'}
    $legacyPath=Join-Path $repo ([string]$e.legacyDisposition).Replace('/','\');if(Test-Path $legacyPath){$rows=@(Import-Csv -LiteralPath $legacyPath -Delimiter "`t");Assert-SameSet @($rows|ForEach-Object{[string]$_.risk_id}) @('P6-D031','P6-D034','P6-D042') 'legacy evidence IDs';foreach($r in $rows){Assert-True([int]$r.active_graph_reachable -eq 0 -and [string]$r.repository_wide_zero_consumer_claim -eq 'FALSE')"legacy claim failed: $($r.risk_id)"}}else{Add-Failure 'legacy evidence missing'}
    $completion=Read-Json $completionPath;if($null-ne$completion){Assert-True([string]$completion.taskId -eq 'P7.1-003' -and [string]$completion.state -eq 'P7_1_003_PASS_COMMIT_ELIGIBLE' -and [string]$completion.expectedParent -eq $expectedParent)'completion identity mismatch';Assert-SameSet @($completion.commitAllowlist) $script:Allowlist 'completion allowlist';Assert-SameSet @($completion.protectedWorktreeValidationProfiles) @('EXACT_PRESERVED_167','CLEAN_CHECKOUT_0') 'completion protected-worktree profiles';Assert-True([int]$completion.rustWorkspacePackages -eq 15 -and [int]$completion.rustPublicSourcePackages -eq 12 -and [int]$completion.rustPublicSourceFiles -ge 12 -and [int]$completion.frontendEntries -eq 2 -and [int]$completion.p4SourceRows -eq 18 -and [int]$completion.rustP4SourceRows -eq 18 -and [int]$completion.frontendP4SourceRows -eq 18 -and [int]$completion.p4SourceMarkers -eq 212 -and [int]$completion.protectedBaselineRows -eq 167)'completion counts mismatch';Assert-True([string]$completion.lockHashes.cargoSha256 -eq (Get-CanonicalTextSha256 (Join-Path $repo 'Cargo.lock')) -and [string]$completion.lockHashes.frontendNpmSha256 -eq (Get-CanonicalTextSha256 (Join-Path $repo 'frontend\package-lock.json')))'completion lock hash mismatch';Assert-True(-not[bool]$completion.repositoryWideZeroConsumerClaim -and [string]$completion.nextTask -eq 'P7.1-004')'completion boundary mismatch';$md=Join-Path $p7 'records\p7-1-003-completion.md';if(Test-Path $md){Assert-True([string]$completion.completionMarkdownSha256 -eq (Get-CanonicalTextSha256 $md))'completion Markdown hash mismatch'}}
}
function Test-PublicationText([string]$Content,[string]$Label){
    $patterns=@(
      @{id='private-key';value='-----BEGIN [A-Z ]*PRIVATE KEY-----'},@{id='ssh-key';value='(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}'},@{id='github-token';value='github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}'},@{id='npm-token';value='npm_[A-Za-z0-9]{20,}'},@{id='openai-token';value='sk-(proj-)?[A-Za-z0-9_-]{20,}'},@{id='jwt';value='eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}'},@{id='credential-url';value='(?i)https?://[^/\s:@]+:[^/\s@]+@'},@{id='windows-absolute';value='(?i)(^|[\s"''])[A-Z]:[\\/]'},@{id='posix-user-path';value='(?i)/(home|Users|mnt/[a-z])/[^/\s"'']+/'},@{id='email';value='(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}'}
    )
    foreach($p in $patterns){Assert-True(-not[regex]::IsMatch($Content,[string]$p.value))"publication rule $($p.id) in $Label"}
}
function Test-PublicationBlobs([string]$Source){
    foreach($path in $script:Allowlist){
        $spec=if($Source -eq 'index'){":$path"}else{"$Source`:$path"};$exists=Invoke-Git @('cat-file','-e',$spec) $true;if($exists.ExitCode -ne 0){continue}
        $blob=Invoke-Git @('show',$spec) $true;Assert-True($blob.ExitCode -eq 0)"cannot read task blob: $path";if($blob.ExitCode -eq 0){Test-PublicationText (@($blob.Output)-join "`n") "$Source blob $path"}
    }
}

Push-Location $repo
try{
    Test-Contract;Test-PreviousTask;Test-ProtectedBaseline;Test-Evidence
    if($null-ne$script:Contract -and [bool]$script:Contract.commitBoundary.finalized){
        $powershellExe=Join-Path $PSHOME 'powershell.exe';$builder=@(& $powershellExe -NoProfile -ExecutionPolicy Bypass -File $builderPath -VerifyOnly 2>&1);$builderCode=$LASTEXITCODE;Assert-True($builderCode -eq 0)"evidence builder VerifyOnly failed: $((@($builder)|Select-Object -Last 8)-join ' | ')"
    }
    $branch=((Invoke-Git @('branch','--show-current')).Output-join'').Trim();Assert-True($branch -eq $expectedBranch)"branch mismatch: $branch"
    $head=((Invoke-Git @('rev-parse','HEAD')).Output-join'').Trim();$remote=Invoke-Git @('rev-parse',"refs/remotes/origin/$expectedBranch") $true;$remoteHead=if($remote.ExitCode-eq0){($remote.Output-join'').Trim()}else{''};Assert-True($remote.ExitCode-eq0)'remote tracking ref unavailable'
    if($Mode -eq 'PreCommit'){
        Assert-True($head -eq $expectedParent)"PreCommit HEAD mismatch: $head";Assert-True($remoteHead -eq $expectedParent)"PreCommit remote mismatch: $remoteHead";$paths=Get-IndexPaths;Assert-SameSet $paths $script:Allowlist 'staged allowlist';$unstaged=@((Invoke-Git (@('diff','--name-only','--')+$script:Allowlist)).Output|Where-Object{$_-ne''});Assert-True($unstaged.Count-eq0)"unstaged task paths: $($unstaged-join',')";Test-PublicationBlobs 'index'
    }else{
        Assert-True($head-eq$primaryCommit)"$Mode must validate immutable primary commit: $head";$parent=((Invoke-Git @('rev-parse','HEAD^')).Output-join'').Trim();Assert-True($parent-eq$expectedParent)"$Mode parent mismatch: $parent";Assert-SameSet (Get-CommitPaths 'HEAD') $script:Allowlist "$Mode commit allowlist";$indexPaths=@(Get-IndexPaths);Assert-True($indexPaths.Count-eq0)"$Mode index not empty";$unstaged=@((Invoke-Git (@('diff','--name-only','--')+$script:Allowlist)).Output|Where-Object{$_-ne''});Assert-True($unstaged.Count-eq0)"$Mode unstaged task paths";Test-PublicationBlobs 'HEAD';$message=((Invoke-Git @('log','-1','--format=%B','HEAD')).Output-join"`n");Test-PublicationText $message "$Mode commit message"
        if($Mode-eq'PostCommit'){Assert-True($remoteHead-eq$expectedParent)'remote advanced before PostCommit validation'}
        if($Mode-eq'PostPush'){Assert-True($remoteHead-eq$head)'remote-tracking ref mismatch';$upstream=Invoke-Git @('rev-parse','--abbrev-ref','--symbolic-full-name','@{upstream}') $true;Assert-True($upstream.ExitCode-eq0-and(($upstream.Output-join'').Trim()-eq"origin/$expectedBranch"))'upstream mismatch';$live=Invoke-Git @('ls-remote','--heads','origin',"refs/heads/$expectedBranch") $true;$liveHash=if($live.ExitCode-eq0-and@($live.Output).Count-eq1){(([string]$live.Output[0])-split'\s+')[0]}else{''};Assert-True($liveHash-eq$head)"live remote mismatch: $liveHash"}
    }
}finally{Pop-Location}

if($failures.Count-gt0){Write-Output 'p7_1_003_gate=FAIL';$failures|ForEach-Object{Write-Output "failure=$_"};exit 1}
$state=if($Mode-eq'PreCommit'){'P7_1_003_PASS_PRE_COMMIT'}elseif($Mode-eq'PostCommit'){'P7_1_003_PASS_POST_COMMIT'}else{'P7_1_003_PASS_POST_PUSH'}
Write-Output "validation_mode=$Mode"
Write-Output "validation_state=$state"
Write-Output 'active_task=P7.1-003'
Write-Output "task_allowlist_paths=$($script:Allowlist.Count)"
Write-Output 'rust_workspace_packages=15'
Write-Output 'frontend_entries=2'
Write-Output 'rust_public_source_packages=12'
Write-Output 'rust_p4_source_rows=18'
Write-Output 'frontend_p4_source_rows=18'
Write-Output 'p4_source_markers=212'
Write-Output 'protected_baseline_rows=167'
Write-Output "protected_worktree_rows=$script:ProtectedWorktreeRows"
Write-Output "protected_worktree_profile=$script:ProtectedWorktreeProfile"
Write-Output 'zero_consumer_scope=ACTIVE_P7_TARGET_GRAPH_ONLY'
Write-Output 'repository_wide_zero_consumer_claim=FALSE'
Write-Output 'rutgers_requests=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_003_gate=PASS'
