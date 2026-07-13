[CmdletBinding()]
param(
    [ValidateSet('PreCommit','PostCommit','PostPush')]
    [string]$Mode = 'PreCommit'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$p7 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$contractPath = Join-Path $p7 '02a-p7-1-002-dependency-lock-license-sbom-baseline.json'
$baselinePath = Join-Path $p7 '01-preserved-worktree-manifest.tsv'
$previousCompletionPath = Join-Path $p7 'records\p7-1-001-completion.json'
$completionPath = Join-Path $p7 'records\p7-1-002-completion.json'
$expectedBranch = 'codex/p7-implementation'
$expectedParent = 'f39b033b491b7f22429348df11e3fd6191ef1615'
$failures = [System.Collections.Generic.List[string]]::new()
$pinnedNodeRelative = '.cache/p7-1-002-node-24.18.0-win-x64/runtime/node-v24.18.0-win-x64/node.exe'
$pinnedCargoRelative = '.cache/p7-1-002/rustup/toolchains/1.97.0-x86_64-pc-windows-msvc/bin/cargo.exe'
$expectedTargets = @('x86_64-pc-windows-msvc','x86_64-unknown-linux-gnu')
$platformExcludedKey = 'rust|cargo:webpki-root-certs@1.0.8:crates.io'
$script:Contract = $null
$script:ExpectedComponents = $null
$script:EvidenceState = $null

$expectedAllowlist = @(
    '.gitignore',
    '.node-version',
    'LICENSE',
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    'deny.toml',
    'crates/dependency-baseline/Cargo.toml',
    'crates/dependency-baseline/src/lib.rs',
    'frontend/package.json',
    'frontend/package-lock.json',
    'project-governance/current/p7/02-p7-1-002-dependency-lock-license-sbom-baseline.md',
    'project-governance/current/p7/02a-p7-1-002-dependency-lock-license-sbom-baseline.json',
    'project-governance/current/p7/evidence/p7-1-002/dependency-graph-scope.json',
    'project-governance/current/p7/evidence/p7-1-002/rejected-dependency-consumers.tsv',
    'project-governance/current/p7/evidence/p7-1-002/locked-components.tsv',
    'project-governance/current/p7/evidence/p7-1-002/official-metadata-sources.tsv',
    'project-governance/current/p7/evidence/p7-1-002/license-allowlist.tsv',
    'project-governance/current/p7/evidence/p7-1-002/license-file-hashes.tsv',
    'project-governance/current/p7/evidence/p7-1-002/initial-sbom.cdx.json',
    'project-governance/current/p7/evidence/p7-1-002/advisory-scan.json',
    'project-governance/current/p7/evidence/p7-1-002/toolchain-fingerprint.json',
    'project-governance/current/p7/records/p7-1-002-completion.md',
    'project-governance/current/p7/records/p7-1-002-completion.json',
    'project-governance/current/p7/tools/build-p7-1-002-evidence.ps1',
    'project-governance/current/p7/tools/validate-p7-1-002.ps1'
)

function Add-Failure([string]$Message) { $failures.Add($Message) }
function Assert-True([bool]$Condition, [string]$Message) { if (-not $Condition) { Add-Failure $Message } }
function Normalize-Path([string]$Path) { return $Path.Replace('\','/') }
function Get-Sha256([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash }
function Read-Json([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { Add-Failure "missing JSON: $(Normalize-Path $Path)"; return $null }
    try { return Get-Content -LiteralPath $Path -Raw -Encoding utf8 | ConvertFrom-Json }
    catch { Add-Failure "invalid JSON: $(Normalize-Path $Path): $($_.Exception.Message)"; return $null }
}
function Has-Property([object]$Object, [string]$Name) {
    return $null -ne $Object -and $null -ne $Object.PSObject.Properties[$Name]
}
function Assert-ExactProperties([object]$Object, [string[]]$Expected, [string]$Label) {
    if ($null -eq $Object) { Add-Failure "$Label is null"; return }
    $actual = @($Object.PSObject.Properties | ForEach-Object { [string]$_.Name })
    Assert-SameSet $actual $Expected "$Label properties" $true
}
function Assert-JsonBoolean([object]$Value, [bool]$Expected, [string]$Label) {
    Assert-True ($Value -is [bool]) "$Label must be a JSON boolean"
    if ($Value -is [bool]) { Assert-True ($Value -eq $Expected) "$Label mismatch" }
}
function Assert-JsonInteger([object]$Value, [int64]$Expected, [string]$Label) {
    $isInteger = $Value -is [byte] -or $Value -is [int16] -or $Value -is [int32] -or $Value -is [int64]
    Assert-True $isInteger "$Label must be a JSON integer"
    if ($isInteger) { Assert-True ([int64]$Value -eq $Expected) "$Label mismatch" }
}
function Invoke-Git([string[]]$Arguments, [bool]$AllowFailure = $false) {
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $output = @(& git -C $repo -c core.autocrlf=false @Arguments 2>$null)
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if (-not $AllowFailure -and $code -ne 0) { throw "git $($Arguments -join ' ') failed with exit $code" }
    return [pscustomobject]@{ Output = $output; ExitCode = $code }
}
function Invoke-Program([string]$FilePath, [string[]]$Arguments, [bool]$AllowFailure = $false) {
    if (-not (Test-Path -LiteralPath $FilePath -PathType Leaf)) {
        if (-not $AllowFailure) { Add-Failure "executable is missing: $(Normalize-Path $FilePath)" }
        return [pscustomobject]@{ Output = @(); ExitCode = 9009 }
    }
    $output = @(& $FilePath @Arguments 2>&1)
    $code = $LASTEXITCODE
    if (-not $AllowFailure -and $code -ne 0) { Add-Failure "program failed ($code): $(Normalize-Path $FilePath) $($Arguments -join ' ')" }
    return [pscustomobject]@{ Output = $output; ExitCode = $code }
}
function Assert-SameSet([string[]]$Actual, [string[]]$Expected, [string]$Label, [bool]$RejectDuplicates = $false) {
    $actualSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $expectedSet = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $actualDuplicates = [System.Collections.Generic.List[string]]::new()
    $expectedDuplicates = [System.Collections.Generic.List[string]]::new()
    foreach ($value in @($Actual)) {
        if ($null -eq $value) { $actualDuplicates.Add('<NULL>'); continue }
        if (-not $actualSet.Add([string]$value)) { $actualDuplicates.Add([string]$value) }
    }
    foreach ($value in @($Expected)) {
        if ($null -eq $value) { $expectedDuplicates.Add('<NULL>'); continue }
        if (-not $expectedSet.Add([string]$value)) { $expectedDuplicates.Add([string]$value) }
    }
    if ($RejectDuplicates) {
        Assert-True ($actualDuplicates.Count -eq 0) "$Label has duplicate actual values: $($actualDuplicates -join ',')"
        Assert-True ($expectedDuplicates.Count -eq 0) "$Label has duplicate expected values: $($expectedDuplicates -join ',')"
    }
    $missing = @($expectedSet | Where-Object { -not $actualSet.Contains([string]$_) })
    $extra = @($actualSet | Where-Object { -not $expectedSet.Contains([string]$_) })
    Assert-True ($missing.Count -eq 0 -and $extra.Count -eq 0) "$Label mismatch: missing=[$($missing -join ',')] extra=[$($extra -join ',')]"
}
function Assert-SameSequence([string[]]$Actual, [string[]]$Expected, [string]$Label) {
    Assert-True (@($Actual).Count -eq @($Expected).Count) "$Label count mismatch: $(@($Actual).Count)/$(@($Expected).Count)"
    $limit = [Math]::Min(@($Actual).Count, @($Expected).Count)
    for ($index = 0; $index -lt $limit; $index++) {
        Assert-True ([StringComparer]::Ordinal.Equals([string]$Actual[$index], [string]$Expected[$index])) "$Label item $index mismatch: '$($Actual[$index])'/'$($Expected[$index])'"
    }
}
function Test-SafeRepositoryRelativePath([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $false }
    if ([System.IO.Path]::IsPathRooted($Value)) { return $false }
    if ($Value -match '(^|[\\/])\.\.([\\/]|$)' -or $Value -match '^[A-Za-z]:' -or ($Value.Length -ge 2 -and $Value[0] -eq [char]92 -and $Value[1] -eq [char]92)) { return $false }
    return $true
}
function Read-ExactTsv([string]$Relative, [string[]]$Headers) {
    $path = Join-Path $repo $Relative.Replace('/','\')
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { Add-Failure "missing TSV: $Relative"; return @() }
    $text = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    Assert-True ($text -notmatch "`0") "$Relative contains NUL"
    $lines = @($text -split '\r?\n' | Where-Object { $_ -ne '' })
    if ($lines.Count -eq 0) { Add-Failure "$Relative is empty"; return @() }
    Assert-SameSequence @($lines[0].Split("`t")) $Headers "$Relative headers"
    $rows = @($lines | ConvertFrom-Csv -Delimiter "`t")
    Assert-True ($rows.Count -gt 0) "$Relative has no evidence rows"
    foreach ($row in $rows) { Assert-ExactProperties $row $Headers "$Relative row" }
    return $rows
}
function Get-IndexPaths {
    return @((Invoke-Git @('diff','--cached','--name-only','--diff-filter=ACDMRTUXB')).Output |
        ForEach-Object { Normalize-Path ([string]$_) } | Where-Object { $_ -ne '' })
}
function Get-CommitPaths([string]$Revision) {
    return @((Invoke-Git @('diff-tree','--no-commit-id','--name-only','-r',$Revision)).Output |
        ForEach-Object { Normalize-Path ([string]$_) } | Where-Object { $_ -ne '' })
}
function Test-RequiredFileSet {
    foreach ($relative in $expectedAllowlist) {
        Assert-True (Test-Path -LiteralPath (Join-Path $repo $relative.Replace('/','\')) -PathType Leaf) "missing task file: $relative"
    }
}
function Test-Contract {
    $contract = Read-Json $contractPath
    if ($null -eq $contract) { return }
    $script:Contract = $contract
    Assert-ExactProperties $contract @(
        'schemaVersion','recordId','taskId','state','branch','expectedParent','expectedRemoteBaseline','dependency','license','scope',
        'toolchainPins','dependencyPins','buildToolPins','licensePolicy','activeGraph','evidence','componentClosurePolicy','advisoryPolicy',
        'fingerprintPolicy','completionPolicy','taskCommitAllowlist','protectedBaseline','publicationBoundary','requiredValidationModes','nextTask','nextTaskBlockedUntil'
    ) 'contract'
    Assert-JsonInteger $contract.schemaVersion 1 'contract schemaVersion'
    Assert-True ([string]$contract.recordId -eq 'P7-1-002-DEPENDENCY-BASELINE-2026-07-13-001') 'contract recordId mismatch'
    Assert-True ([string]$contract.taskId -eq 'P7.1-002') 'contract taskId mismatch'
    Assert-True ([string]$contract.state -eq 'IMPLEMENTATION_CONTRACT') 'contract state mismatch'
    Assert-True ([string]$contract.branch -eq $expectedBranch) 'contract branch mismatch'
    Assert-True ([string]$contract.expectedParent -eq $expectedParent) 'contract parent mismatch'
    Assert-True ([string]$contract.expectedRemoteBaseline -eq $expectedParent) 'contract remote baseline mismatch'
    Assert-ExactProperties $contract.dependency @('taskId','requiredState','requiredCommit') 'contract dependency'
    Assert-True ([string]$contract.dependency.taskId -eq 'P7.1-001') 'contract prerequisite task mismatch'
    Assert-True ([string]$contract.dependency.requiredState -eq 'P7_1_001_PASS_COMMIT_ELIGIBLE') 'contract prerequisite state mismatch'
    Assert-True ([string]$contract.dependency.requiredCommit -eq $expectedParent) 'contract prerequisite commit mismatch'
    Assert-ExactProperties $contract.license @('spdx','copyright','file') 'contract license'
    Assert-True ([string]$contract.license.spdx -eq 'ISC') 'contract license must be ISC'
    Assert-True ([string]$contract.license.copyright -eq 'Copyright (c) 2026 VVittgenstein') 'contract copyright mismatch'
    Assert-True ([string]$contract.license.file -eq 'LICENSE') 'contract license file mismatch'
    Assert-ExactProperties $contract.scope @('kind','productBusinessLogicAllowed','finalWorkspaceModuleGraphOwnedBy','zeroConsumerScope','legacyGraphDisposition','repositoryWideZeroConsumerClaim','realRutgersRequestsAllowed','p75ExecutionAuthorized','vultrMutationAllowed','releaseOrProductionAllowed') 'contract scope'
    Assert-True ([string]$contract.scope.kind -eq 'DEPENDENCY_RESOLUTION_SCAFFOLD_ONLY') 'task is not resolution-only'
    Assert-JsonBoolean $contract.scope.productBusinessLogicAllowed $false 'contract productBusinessLogicAllowed'
    Assert-True ([string]$contract.scope.finalWorkspaceModuleGraphOwnedBy -eq 'P7.1-003') 'P7.1-003 workspace ownership missing'
    Assert-True ([string]$contract.scope.zeroConsumerScope -eq 'ACTIVE_P7_TARGET_GRAPH_ONLY') 'zero-consumer scope mismatch'
    Assert-True ([string]$contract.scope.legacyGraphDisposition -eq 'FROZEN_EXCLUDED_PENDING_OWNER_TASK') 'legacy disposition mismatch'
    foreach ($field in @('repositoryWideZeroConsumerClaim','realRutgersRequestsAllowed','p75ExecutionAuthorized','vultrMutationAllowed','releaseOrProductionAllowed')) {
        Assert-JsonBoolean $contract.scope.$field $false "contract scope $field"
    }
    Assert-ExactProperties $contract.toolchainPins @('rust','node','npm') 'contract toolchainPins'
    Assert-True ([string]$contract.toolchainPins.rust -eq '1.97.0') 'Rust pin mismatch'
    Assert-True ([string]$contract.toolchainPins.node -eq '24.18.0') 'Node pin mismatch'
    Assert-True ([string]$contract.toolchainPins.npm -eq '11.16.0') 'npm pin mismatch'
    Assert-ExactProperties $contract.dependencyPins @('time','jiff','jiffRole','jiffTypesMayEnterDomainOrApi') 'contract dependencyPins'
    Assert-True ([string]$contract.dependencyPins.time -eq '0.3.53') 'time pin mismatch'
    Assert-True ([string]$contract.dependencyPins.jiff -eq '0.2.32') 'jiff pin mismatch'
    Assert-True ([string]$contract.dependencyPins.jiffRole -eq 'ENCAPSULATED_AMERICA_NEW_YORK_IANA_DST_ADAPTER') 'Jiff role mismatch'
    Assert-JsonBoolean $contract.dependencyPins.jiffTypesMayEnterDomainOrApi $false 'contract jiffTypesMayEnterDomainOrApi'
    Assert-ExactProperties $contract.buildToolPins @('cargo-deny','cargo-about','cargo-cyclonedx','distribution') 'contract buildToolPins'
    Assert-True ([string]$contract.buildToolPins.'cargo-deny' -eq '0.20.2') 'cargo-deny pin mismatch'
    Assert-True ([string]$contract.buildToolPins.'cargo-about' -eq '0.9.1') 'cargo-about pin mismatch'
    Assert-True ([string]$contract.buildToolPins.'cargo-cyclonedx' -eq '0.5.9') 'cargo-cyclonedx pin mismatch'
    Assert-True ([string]$contract.buildToolPins.distribution -eq 'BUILD_AND_CI_ONLY_NOT_IN_FINAL_PACKAGES') 'build-tool distribution boundary mismatch'

    Assert-ExactProperties $contract.licensePolicy @('rustAllowSource','rustPlatformExcluded','npmAllowed','npmConditional','unknownOrUnresolvedDecision') 'contract licensePolicy'
    Assert-True ([string]$contract.licensePolicy.rustAllowSource -eq 'deny.toml') 'Rust license allow source mismatch'
    Assert-True (@($contract.licensePolicy.rustPlatformExcluded).Count -eq 1) 'contract platform exclusion must be a singleton'
    if (@($contract.licensePolicy.rustPlatformExcluded).Count -eq 1) {
        $excluded = $contract.licensePolicy.rustPlatformExcluded[0]
        Assert-ExactProperties $excluded @('name','version','license','decision','basis') 'contract platform exclusion'
        Assert-True ([string]$excluded.name -eq 'webpki-root-certs') 'platform exclusion name mismatch'
        Assert-True ([string]$excluded.version -eq '1.0.8') 'platform exclusion version mismatch'
        Assert-True ([string]$excluded.license -eq 'CDLA-Permissive-2.0') 'platform exclusion license mismatch'
        Assert-True ([string]$excluded.decision -eq 'PLATFORM_EXCLUDED') 'platform exclusion decision mismatch'
        Assert-True ([string]$excluded.basis -eq 'UNREACHABLE_FROM_X86_64_PC_WINDOWS_MSVC_AND_X86_64_UNKNOWN_LINUX_GNU_ACTIVE_TARGET_GRAPHS') 'platform exclusion basis mismatch'
    }
    $expectedNpmLicenses = @('0BSD','Apache-2.0','BlueOak-1.0.0','BSD-2-Clause','BSD-3-Clause','CC0-1.0','ISC','MIT','MIT-0','MPL-2.0')
    Assert-SameSet @($contract.licensePolicy.npmAllowed) $expectedNpmLicenses 'npm license policy' $true
    Assert-ExactProperties $contract.licensePolicy.npmConditional @('MPL-2.0','CC-BY-4.0') 'contract npmConditional'
    Assert-True ([string]$contract.licensePolicy.npmConditional.'MPL-2.0' -eq 'DEV_ONLY_UNMODIFIED_EXCLUDED_FROM_FINAL_RUNTIME_BUNDLES') 'MPL-2.0 scope restriction missing'
    Assert-True ([string]$contract.licensePolicy.npmConditional.'CC-BY-4.0' -eq 'BUILD_ONLY_ATTRIBUTION_REQUIRED_IF_ENCOUNTERED') 'CC-BY-4.0 restriction missing'
    Assert-True ([string]$contract.licensePolicy.unknownOrUnresolvedDecision -eq 'STOP') 'unknown license stop rule missing'

    Assert-ExactProperties $contract.activeGraph @('manifests','locks','legacyExcludedManifests','embedCandidatePolicy') 'contract activeGraph'
    Assert-SameSequence @($contract.activeGraph.manifests) @('Cargo.toml','crates/dependency-baseline/Cargo.toml','frontend/package.json') 'active manifests'
    Assert-SameSequence @($contract.activeGraph.locks) @('Cargo.lock','frontend/package-lock.json') 'active locks'
    Assert-SameSequence @($contract.activeGraph.legacyExcludedManifests) @('package.json','package-lock.json') 'excluded legacy manifests'
    Assert-True ([string]$contract.activeGraph.embedCandidatePolicy -eq 'EXACTLY_ONE_OF_INCLUDE_DIR_OR_RUST_EMBED_AFTER_MECHANICAL_COMPARISON') 'embed policy mismatch'
    $expectedEvidence = [ordered]@{
        graphScope='project-governance/current/p7/evidence/p7-1-002/dependency-graph-scope.json'; rejectedConsumers='project-governance/current/p7/evidence/p7-1-002/rejected-dependency-consumers.tsv'
        lockedComponents='project-governance/current/p7/evidence/p7-1-002/locked-components.tsv'; officialMetadataSources='project-governance/current/p7/evidence/p7-1-002/official-metadata-sources.tsv'
        licenseAllowlist='project-governance/current/p7/evidence/p7-1-002/license-allowlist.tsv'; licenseFileHashes='project-governance/current/p7/evidence/p7-1-002/license-file-hashes.tsv'
        initialSbom='project-governance/current/p7/evidence/p7-1-002/initial-sbom.cdx.json'; advisoryScan='project-governance/current/p7/evidence/p7-1-002/advisory-scan.json'
        toolchainFingerprint='project-governance/current/p7/evidence/p7-1-002/toolchain-fingerprint.json'
    }
    Assert-ExactProperties $contract.evidence @($expectedEvidence.Keys) 'contract evidence'
    foreach ($name in $expectedEvidence.Keys) { Assert-True ([string]$contract.evidence.$name -eq [string]$expectedEvidence[$name]) "contract evidence path mismatch: $name" }

    Assert-ExactProperties $contract.componentClosurePolicy @('identity','includeCargoWorkspaceMember','includeFrontendRootPackage','requiredExactSets','licenseFileAbsence','cycloneDx') 'contract componentClosurePolicy'
    Assert-True ([string]$contract.componentClosurePolicy.identity -eq 'ORDINAL_ECOSYSTEM_AND_CANONICAL_LOCK_KEY') 'component identity policy mismatch'
    Assert-JsonBoolean $contract.componentClosurePolicy.includeCargoWorkspaceMember $true 'includeCargoWorkspaceMember'
    Assert-JsonBoolean $contract.componentClosurePolicy.includeFrontendRootPackage $true 'includeFrontendRootPackage'
    Assert-SameSequence @($contract.componentClosurePolicy.requiredExactSets) @('LOCKED_COMPONENTS','OFFICIAL_METADATA','LICENSE_DECISIONS','LICENSE_HASH_COVERAGE','CYCLONEDX_COMPONENTS') 'component exact sets'
    Assert-ExactProperties $contract.componentClosurePolicy.licenseFileAbsence @('licenseFile','sha256','evidenceStatus','reason') 'licenseFileAbsence'
    Assert-True ([string]$contract.componentClosurePolicy.licenseFileAbsence.licenseFile -eq 'NO_PACKAGED_LICENSE_FILE') 'license-file sentinel mismatch'
    Assert-True ([string]$contract.componentClosurePolicy.licenseFileAbsence.sha256 -eq 'NOT_APPLICABLE') 'license hash sentinel mismatch'
    Assert-True ([string]$contract.componentClosurePolicy.licenseFileAbsence.evidenceStatus -eq 'ABSENT_WITH_EXPLICIT_DECISION') 'license evidence-status sentinel mismatch'
    Assert-True ([string]$contract.componentClosurePolicy.licenseFileAbsence.reason -eq 'NO_LICENSE_FILE_IN_VERIFIED_PACKAGE_ARCHIVE') 'license reason sentinel mismatch'
    Assert-ExactProperties $contract.componentClosurePolicy.cycloneDx @('specVersion','schemaValidation','bomRefUnique') 'componentClosurePolicy cycloneDx'
    Assert-True ([string]$contract.componentClosurePolicy.cycloneDx.specVersion -eq '1.6') 'CycloneDX contract spec mismatch'
    Assert-True ([string]$contract.componentClosurePolicy.cycloneDx.schemaValidation -eq 'EMBEDDED_STRICT_CYCLONEDX_1_6_STRUCTURAL_SCHEMA_AND_TOOL_CROSS_CHECK') 'CycloneDX validation policy mismatch'
    Assert-JsonBoolean $contract.componentClosurePolicy.cycloneDx.bomRefUnique $true 'CycloneDX bomRefUnique'

    Assert-ExactProperties $contract.advisoryPolicy @('networkRequiredAtCapture','rawOutputMayBePublished','requiredCommands','cargoDenyArguments','npmAuditArguments','requiredExitCode','requiredVulnerabilities') 'contract advisoryPolicy'
    Assert-JsonBoolean $contract.advisoryPolicy.networkRequiredAtCapture $true 'advisory networkRequiredAtCapture'
    Assert-JsonBoolean $contract.advisoryPolicy.rawOutputMayBePublished $false 'advisory rawOutputMayBePublished'
    Assert-SameSequence @($contract.advisoryPolicy.requiredCommands) @('cargo-deny-check','npm-audit-package-lock') 'advisory command IDs'
    Assert-SameSequence @($contract.advisoryPolicy.cargoDenyArguments) @('--all-features','check','advisories','bans','licenses','sources') 'cargo-deny arguments'
    Assert-SameSequence @($contract.advisoryPolicy.npmAuditArguments) @('audit','--prefix','frontend','--package-lock-only','--json') 'npm audit arguments'
    Assert-JsonInteger $contract.advisoryPolicy.requiredExitCode 0 'advisory requiredExitCode'
    Assert-JsonInteger $contract.advisoryPolicy.requiredVulnerabilities 0 'advisory requiredVulnerabilities'

    Assert-ExactProperties $contract.fingerprintPolicy @('pathsAreRepositoryRelative','rawCommandOutputMayBePublished','nodeDistributionSha256','requiredBinaries','requiredCommandIds') 'contract fingerprintPolicy'
    Assert-JsonBoolean $contract.fingerprintPolicy.pathsAreRepositoryRelative $true 'fingerprint pathsAreRepositoryRelative'
    Assert-JsonBoolean $contract.fingerprintPolicy.rawCommandOutputMayBePublished $false 'fingerprint rawCommandOutputMayBePublished'
    Assert-True ([string]$contract.fingerprintPolicy.nodeDistributionSha256 -eq '0AE68406B42D7725661DA979B1403EC9926DA205C6770827F33AAC9D8F26E821') 'Node distribution hash mismatch'
    $expectedBinaries = @(
        [pscustomobject]@{id='rustc';path='.cache/p7-1-002/rustup/toolchains/1.97.0-x86_64-pc-windows-msvc/bin/rustc.exe';version='1.97.0';sha256='6D1C5543ED3A45CFBC1C1332D42D6550D883C14D3C2E323427E631C331CEBEEB'},
        [pscustomobject]@{id='cargo';path=$pinnedCargoRelative;version='1.97.0';sha256='3CD119FE81DFEDB9DCE4573696BF65058F16B57C9E5BABE415B71624315CBB7D'},
        [pscustomobject]@{id='node';path=$pinnedNodeRelative;version='24.18.0';sha256='9A4EB5F1C29C6A2E93852EAD46B999E284A6A5CA8BAB4D4E241D587D025A52DE'},
        [pscustomobject]@{id='npm';path='.cache/p7-1-002-node-24.18.0-win-x64/runtime/node-v24.18.0-win-x64/npm.cmd';version='11.16.0';sha256='21B46C69AD6E2F231F02A9E120F4BA6C8E75FEF5A45637103002EAB99F888AB8'},
        [pscustomobject]@{id='cargo-deny';path='.cache/p7-1-002/tools/cargo-deny/bin/cargo-deny.exe';version='0.20.2';sha256='1777463F5ECCCA98AD164D07C3B256B8F882BF0F9A83C5D2AD9852A1E0EDAABD'},
        [pscustomobject]@{id='cargo-about';path='.cache/p7-1-002/tools/cargo-about/bin/cargo-about.exe';version='0.9.1';sha256='732559B990D44E53FAE83DFA1B3DBC1FFDFC59F64E328517DBB41B035A53C33B'},
        [pscustomobject]@{id='cargo-cyclonedx';path='.cache/p7-1-002/tools/cargo-cyclonedx/bin/cargo-cyclonedx.exe';version='0.5.9';sha256='13976AF171C18FD567D0233361D90D967C8C12565990F7E7462C2FA8ABEC67E6'}
    )
    Assert-SameSet @($contract.fingerprintPolicy.requiredBinaries | ForEach-Object { [string]$_.id }) @($expectedBinaries.id) 'fingerprint required binary IDs' $true
    foreach ($expected in $expectedBinaries) {
        $matches = @($contract.fingerprintPolicy.requiredBinaries | Where-Object { [StringComparer]::Ordinal.Equals([string]$_.id,[string]$expected.id) })
        Assert-True ($matches.Count -eq 1) "fingerprint contract binary multiplicity: $($expected.id)"
        if ($matches.Count -eq 1) {
            $binary = $matches[0]
            Assert-ExactProperties $binary @('id','path','version','sha256') "fingerprint contract binary $($expected.id)"
            foreach ($field in @('path','version','sha256')) { Assert-True ([string]$binary.$field -eq [string]$expected.$field) "fingerprint contract binary $($expected.id) $field mismatch" }
        }
    }
    Assert-SameSequence @($contract.fingerprintPolicy.requiredCommandIds) @('rustc-version','cargo-version','node-version','npm-version','cargo-deny-version','cargo-about-version','cargo-cyclonedx-help') 'fingerprint required command IDs'
    Assert-ExactProperties $contract.completionPolicy @('requiredQualityGateIds','actualCommitExcludedToAvoidSelfReference','postPushState') 'contract completionPolicy'
    Assert-SameSequence @($contract.completionPolicy.requiredQualityGateIds) @('cargo-check-locked-offline','cargo-test-locked-offline','cargo-clippy-locked-offline','cargo-fmt-check','cargo-deny-check','npm-ci-isolated','npm-ls-isolated','npm-dedupe-dry-run-isolated','evidence-builder-verify','evidence-builder-write') 'completion quality gate IDs'
    Assert-JsonBoolean $contract.completionPolicy.actualCommitExcludedToAvoidSelfReference $true 'completion actualCommitExcludedToAvoidSelfReference'
    Assert-True ([string]$contract.completionPolicy.postPushState -eq 'P7_1_002_PASS_POST_PUSH') 'completion postPushState mismatch'

    Assert-True ($expectedAllowlist.Count -eq 26) 'hardcoded task allowlist is not 26 paths'
    Assert-SameSet $expectedAllowlist $expectedAllowlist 'hardcoded task allowlist uniqueness' $true
    Assert-True (@($contract.taskCommitAllowlist).Count -eq 26) 'contract task allowlist is not 26 paths'
    Assert-SameSet @($contract.taskCommitAllowlist) $expectedAllowlist 'contract task allowlist' $true
    Assert-ExactProperties $contract.protectedBaseline @('manifest','expectedRows','opaqueRowsMustNotBeRead','privateRoot','privateRootPolicy') 'contract protectedBaseline'
    Assert-True ([string]$contract.protectedBaseline.manifest -eq 'project-governance/current/p7/01-preserved-worktree-manifest.tsv') 'protected manifest path mismatch'
    Assert-JsonInteger $contract.protectedBaseline.expectedRows 167 'protected baseline row count'
    Assert-JsonBoolean $contract.protectedBaseline.opaqueRowsMustNotBeRead $true 'opaqueRowsMustNotBeRead'
    Assert-True ([string]$contract.protectedBaseline.privateRoot -eq '.secrets/') 'private root mismatch'
    Assert-True ([string]$contract.protectedBaseline.privateRootPolicy -eq 'OPAQUE_PRIVATE_DENY_NO_ENUMERATION') 'private root policy mismatch'
    Assert-ExactProperties $contract.publicationBoundary @('preauthorizedMetadata','forbidden','expandedPublicScopeRequiresFreshConfirmation') 'contract publicationBoundary'
    Assert-SameSequence @($contract.publicationBoundary.preauthorizedMetadata) @('REPOSITORY_RELATIVE_PATH','GIT_STATUS','FILE_SIZE','NON_SENSITIVE_HASH') 'preauthorized publication metadata'
    Assert-SameSequence @($contract.publicationBoundary.forbidden) @('FILE_CONTENT_DUMP','SECRETS_OR_CREDENTIALS','PERSONAL_INFORMATION','ABSOLUTE_USER_PATH','RAW_REAL_CATALOG_OR_OPEN_PAYLOAD','PRIVATE_INFRASTRUCTURE_INVENTORY') 'forbidden publication classes'
    Assert-JsonBoolean $contract.publicationBoundary.expandedPublicScopeRequiresFreshConfirmation $true 'expandedPublicScopeRequiresFreshConfirmation'
    Assert-SameSequence @($contract.requiredValidationModes) @('PreCommit','PostCommit','PostPush') 'required validation modes'
    Assert-True ([string]$contract.nextTask -eq 'P7.1-003') 'contract nextTask mismatch'
    Assert-True ([string]$contract.nextTaskBlockedUntil -eq 'P7_1_002_PASS_POST_PUSH') 'contract nextTask blocker mismatch'
}
function Test-PreviousTask {
    $previous = Read-Json $previousCompletionPath
    if ($null -ne $previous) {
        Assert-True ([string]$previous.taskId -eq 'P7.1-001') 'previous completion task mismatch'
        Assert-True ([string]$previous.state -eq 'P7_1_001_PASS_COMMIT_ELIGIBLE') 'P7.1-001 is not complete'
        Assert-True ([int]$previous.manifestRows -eq 167) 'P7.1-001 manifest row count mismatch'
    }
    $exists = Invoke-Git @('cat-file','-e',"$expectedParent^{commit}") $true
    Assert-True ($exists.ExitCode -eq 0) 'expected P7.1-001 commit is unavailable'
    if ($exists.ExitCode -eq 0) {
        $previousPaths = Get-CommitPaths $expectedParent
        $expectedPrevious = @(
            'project-governance/current/p7/00-p7-authority-and-scope.md',
            'project-governance/current/p7/00a-p7-authority-and-scope.json',
            'project-governance/current/p7/01-preserved-worktree-manifest.tsv',
            'project-governance/current/p7/records/p7-1-001-completion.md',
            'project-governance/current/p7/records/p7-1-001-completion.json',
            'project-governance/current/p7/tools/validate-p7-1-001.ps1'
        )
        Assert-SameSet $previousPaths $expectedPrevious 'P7.1-001 committed paths'
    }
}
function Test-ProtectedBaseline {
    if (-not (Test-Path -LiteralPath $baselinePath -PathType Leaf)) { Add-Failure 'protected baseline manifest is missing'; return }
    $baseline = @(Import-Csv -LiteralPath $baselinePath -Delimiter "`t")
    Assert-True ($baseline.Count -eq 167) "protected baseline has $($baseline.Count) rows, expected 167"
    $allow = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($path in $expectedAllowlist) { [void]$allow.Add($path) }
    $actual = @{}
    foreach ($raw in @((Invoke-Git @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames')).Output)) {
        $line = [string]$raw
        if ($line.Length -lt 4) { continue }
        $code = $line.Substring(0,2)
        $path = Normalize-Path $line.Substring(3)
        if ($allow.Contains($path)) { continue }
        if ($code[0] -ne ' ' -and $code -ne '??') { Add-Failure "protected/foreign path is staged: $path ($code)" }
        $status = if ($code -eq '??') { 'UNTRACKED' } elseif ($code[1] -eq 'D') { 'WORKTREE_D' } else { "WORKTREE_$($code[1])" }
        $actual[$path] = $status
    }
    Assert-SameSet @($actual.Keys) @($baseline.path) 'protected worktree path set'
    foreach ($row in $baseline) {
        $path = [string]$row.path
        if (-not $actual.ContainsKey($path)) { continue }
        Assert-True ([string]$actual[$path] -eq [string]$row.git_status) "protected status drift: $path"
        $absolute = Join-Path $repo $path.Replace('/','\')
        if ([string]$row.git_status -eq 'WORKTREE_D') {
            Assert-True (-not (Test-Path -LiteralPath $absolute)) "protected deleted path reappeared: $path"
            $blob = ((Invoke-Git @('rev-parse',"HEAD:$path")).Output -join '').Trim()
            Assert-True ($blob -eq [string]$row.head_blob_oid) "protected HEAD blob drift: $path"
            continue
        }
        Assert-True (Test-Path -LiteralPath $absolute -PathType Leaf) "protected file is missing: $path"
        if (Test-Path -LiteralPath $absolute -PathType Leaf) {
            Assert-True ((Get-Item -LiteralPath $absolute).Length -eq [int64]$row.size_bytes) "protected size drift: $path"
            if ([string]$row.worktree_sha256 -ne 'NOT_COMPUTED') {
                Assert-True ((Get-Sha256 $absolute) -eq [string]$row.worktree_sha256) "protected content drift: $path"
            } else {
                Assert-True ([string]$row.content_policy -eq 'OPAQUE_PROTECTED_NO_READ') "opaque protection marker drift: $path"
            }
        }
    }
    $ignored = Invoke-Git @('check-ignore','-q','.secrets/') $true
    Assert-True ($ignored.ExitCode -eq 0) 'private root is not ignored'
    $trackedPrivate = @((Invoke-Git @('ls-files','--','.secrets')).Output | Where-Object { $_ -ne '' })
    Assert-True ($trackedPrivate.Count -eq 0) 'private root contains tracked paths'
}
function Test-LicenseAndPins {
    $license = Get-Content -LiteralPath (Join-Path $repo 'LICENSE') -Raw -Encoding utf8
    Assert-True ($license.StartsWith("ISC License`n") -or $license.StartsWith("ISC License`r`n")) 'LICENSE is not the ISC text'
    Assert-True ($license -match [regex]::Escape('Copyright (c) 2026 VVittgenstein')) 'LICENSE copyright mismatch'
    Assert-True ($license -match 'Permission to use, copy, modify, and/or distribute this software') 'ISC grant is incomplete'
    Assert-True ((Get-Content -LiteralPath (Join-Path $repo '.node-version') -Raw -Encoding utf8).Trim() -eq '24.18.0') '.node-version mismatch'
    $toolchain = Get-Content -LiteralPath (Join-Path $repo 'rust-toolchain.toml') -Raw -Encoding utf8
    Assert-True ($toolchain -match '(?m)^channel\s*=\s*"1\.97\.0"\s*$') 'rust-toolchain channel mismatch'
    Assert-True ($toolchain -match '(?m)^profile\s*=\s*"minimal"\s*$') 'rust-toolchain profile mismatch'
    Assert-True ($toolchain -match 'rustfmt' -and $toolchain -match 'clippy') 'rustfmt/clippy components missing'
    $cargo = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw -Encoding utf8
    Assert-True ($cargo -match '(?m)^resolver\s*=\s*"3"\s*$') 'Cargo resolver must be 3'
    Assert-True ($cargo -match '(?m)^rust-version\s*=\s*"1\.97\.0"\s*$') 'workspace rust-version mismatch'
    Assert-True ($cargo -match '(?m)^license\s*=\s*"ISC"\s*$') 'workspace license mismatch'
    Assert-True ($cargo -match '(?m)^time\s*=.*version\s*=\s*"=0\.3\.53"') 'time must be pinned exactly to 0.3.53'
    Assert-True ($cargo -match '(?m)^jiff\s*=.*version\s*=\s*"=0\.2\.32"') 'jiff must be pinned exactly to 0.2.32'
    Assert-True ($cargo -match '(?m)^sha2\s*=.*version\s*=\s*"=0\.10\.9"') 'sha2 compatibility pin mismatch'
    Assert-True ($cargo -match '(?m)^tower-http\s*=.*version\s*=\s*"=0\.6\.11"') 'tower-http compatibility pin mismatch'
    Assert-True ($cargo -match '(?m)^reqwest\s*=.*features\s*=.*"gzip".*"rustls"') 'reqwest must use gzip and rustls features'
    Assert-True ($cargo -match '(?m)^tower-http\s*=.*features\s*=.*"compression-gzip".*"fs".*"limit".*"trace"') 'tower-http feature closure is incomplete'
    Assert-True ($cargo -match '(?m)^cargo-deny\s*=\s*"0\.20\.2"\s*$') 'cargo-deny metadata pin mismatch'
    Assert-True ($cargo -match '(?m)^cargo-about\s*=\s*"0\.9\.1"\s*$') 'cargo-about metadata pin mismatch'
    Assert-True ($cargo -match '(?m)^cargo-cyclonedx\s*=\s*"0\.5\.9"\s*$') 'cargo-cyclonedx metadata pin mismatch'
    Assert-True ($cargo -notmatch '(?m)^\s*directories\s*=') 'rejected directories crate is active'
    foreach ($sectionName in @('workspace.dependencies')) {
        $section = [regex]::Match($cargo, "(?ms)^\[$([regex]::Escape($sectionName))\]\s*(.*?)(?=^\[[^\r\n]+\]|\z)")
        Assert-True $section.Success "Cargo.toml lacks [$sectionName]"
        if ($section.Success) {
            foreach ($line in @($section.Groups[1].Value -split '\r?\n' | Where-Object { $_ -match '^\s*[A-Za-z0-9_-]+\s*=' })) {
                $exactStringPin = $line -match '=\s*"=\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?"\s*$'
                $exactTablePin = $line -match 'version\s*=\s*"=\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?"'
                Assert-True ($exactStringPin -or $exactTablePin) "Cargo direct dependency is not an exact pin: $($line.Trim())"
                Assert-True ($line -notmatch '\b(git|branch|rev|path)\s*=') "Cargo direct dependency uses a non-registry source: $($line.Trim())"
            }
        }
    }
    $lockText = Get-Content -LiteralPath (Join-Path $repo 'Cargo.lock') -Raw -Encoding utf8
    Assert-True ($lockText -notmatch '(?m)^source\s*=\s*"git\+') 'floating Git source exists in Cargo.lock'
    Assert-True ($lockText -match '(?ms)^name\s*=\s*"time"\s*\r?\nversion\s*=\s*"0\.3\.53"') 'Cargo.lock time pin mismatch'
    Assert-True ($lockText -match '(?ms)^name\s*=\s*"jiff"\s*\r?\nversion\s*=\s*"0\.2\.32"') 'Cargo.lock jiff pin mismatch'
    Assert-True ($lockText -match '(?ms)^name\s*=\s*"sha2"\s*\r?\nversion\s*=\s*"0\.10\.9"') 'Cargo.lock sha2 pin mismatch'
    Assert-True ($lockText -match '(?ms)^name\s*=\s*"tower-http"\s*\r?\nversion\s*=\s*"0\.6\.11"') 'Cargo.lock tower-http pin mismatch'
    Assert-True ($lockText -notmatch '(?m)^name\s*=\s*"directories"\s*$') 'rejected directories crate exists in Cargo.lock'
    Assert-True ($lockText -notmatch '(?m)^name\s*=\s*"(native-tls|openssl|openssl-sys)"\s*$') 'native TLS/OpenSSL crate exists in Cargo.lock'
    $deny = Get-Content -LiteralPath (Join-Path $repo 'deny.toml') -Raw -Encoding utf8
    Assert-True ($deny -match '(?m)^multiple-versions\s*=\s*"deny"\s*$') 'duplicate policy must default to deny'
    $skip = [regex]::Match($deny, '(?ms)^skip\s*=\s*\[(.*?)\]')
    Assert-True $skip.Success 'deny.toml bans.skip is missing'
    if ($skip.Success) {
        $skipEntries = @([regex]::Matches($skip.Groups[1].Value,'\{([^{}]+)\}'))
        Assert-True ($skipEntries.Count -eq 1) 'deny.toml duplicate skip must contain exactly one exception'
        if ($skipEntries.Count -eq 1) {
            $body = $skipEntries[0].Groups[1].Value
            Assert-True ($body -match '^\s*name\s*=\s*"getrandom"\s*,\s*version\s*=\s*"=0\.3\.4"\s*,?\s*$') 'getrandom duplicate exception is missing or broadened'
        }
    }
    $skipTree = [regex]::Match($deny, '(?ms)^skip-tree\s*=\s*\[(.*?)\]')
    if ($skipTree.Success) { Assert-True ([string]::IsNullOrWhiteSpace($skipTree.Groups[1].Value)) 'deny.toml skip-tree must be empty' }
    $cargoHome = Join-Path $repo '.cache\p7-1-002\cargo'
    $rustupHome = Join-Path $repo '.cache\p7-1-002\rustup'
    $oldCargoHome = $env:CARGO_HOME; $oldRustupHome = $env:RUSTUP_HOME; $oldCargoOffline = $env:CARGO_NET_OFFLINE; $oldRustupToolchain = $env:RUSTUP_TOOLCHAIN; $oldPath = $env:Path
    try {
        $env:CARGO_HOME = $cargoHome; $env:RUSTUP_HOME = $rustupHome; $env:CARGO_NET_OFFLINE = 'true'; $env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'; $env:Path = (Join-Path $cargoHome 'bin') + [IO.Path]::PathSeparator + $oldPath
        $denyRun = Invoke-Program (Join-Path $repo '.cache\p7-1-002\tools\cargo-deny\bin\cargo-deny.exe') @('--all-features','check','bans','licenses','sources') $true
        Assert-True ($denyRun.ExitCode -eq 0) 'offline cargo-deny bans/licenses/sources gate failed'
    } finally {
        $env:CARGO_HOME = $oldCargoHome; $env:RUSTUP_HOME = $oldRustupHome; $env:CARGO_NET_OFFLINE = $oldCargoOffline; $env:RUSTUP_TOOLCHAIN = $oldRustupToolchain; $env:Path = $oldPath
    }
    $licenseSection = [regex]::Match($deny, '(?ms)^\[licenses\]\s*(.*?)(?=^\[[^\r\n]+\]|\z)')
    Assert-True $licenseSection.Success 'deny.toml licenses section missing'
    if ($licenseSection.Success) {
        $allow = [regex]::Match($licenseSection.Groups[1].Value, '(?ms)^allow\s*=\s*\[(.*?)\]')
        Assert-True $allow.Success 'deny.toml licenses.allow missing'
        if ($allow.Success) {
            $actualRustLicenses = @([regex]::Matches($allow.Groups[1].Value,'"([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
            Assert-SameSet $actualRustLicenses @('0BSD','Apache-2.0','BSD-2-Clause','BSD-3-Clause','CC0-1.0','ISC','MIT','MIT-0','Unicode-3.0','Unlicense','Zlib') 'Rust license allow policy' $true
        }
    }
    $graphSection = [regex]::Match($deny, '(?ms)^\[graph\]\s*(.*?)(?=^\[[^\r\n]+\]|\z)')
    Assert-True $graphSection.Success 'deny.toml graph section missing'
    if ($graphSection.Success) {
        $targets = @([regex]::Matches($graphSection.Groups[1].Value,'triple\s*=\s*"([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
        Assert-SameSet $targets $expectedTargets 'deny.toml approved targets' $true
    }
    $embedCount = @(@('include_dir','rust-embed') | Where-Object { $lockText -match "(?m)^name\s*=\s*`"$([regex]::Escape($_))`"\s*$" }).Count
    Assert-True ($embedCount -eq 1) "active embed candidate count is $embedCount, expected 1"
    $lib = Get-Content -LiteralPath (Join-Path $repo 'crates/dependency-baseline/src/lib.rs') -Raw -Encoding utf8
    Assert-True ([Text.Encoding]::UTF8.GetByteCount($lib) -le 8192) 'dependency scaffold source is unexpectedly large'
    Assert-True ($lib -notmatch '(?im)\b(Rutgers|Course|Section|openSections|TcpListener|Router::|CREATE TABLE)\b') 'product behavior token found in resolution scaffold'
    Assert-True ($lib -notmatch '(?m)^\s*(pub\s+)?(async\s+)?(fn|struct|enum|trait|impl)\b') 'business/code declarations found in resolution scaffold'

    $frontend = Read-Json (Join-Path $repo 'frontend\package.json')
    $frontendLockPath = Join-Path $repo 'frontend\package-lock.json'
    $frontendLockText = if (Test-Path -LiteralPath $frontendLockPath -PathType Leaf) { Get-Content -LiteralPath $frontendLockPath -Raw -Encoding utf8 } else { '' }
    $nodePath = Join-Path $repo $pinnedNodeRelative.Replace('/','\')
    $nodeVersion = Invoke-Program $nodePath @('--version') $true
    Assert-True ($nodeVersion.ExitCode -eq 0 -and (($nodeVersion.Output -join '').Trim() -eq 'v24.18.0')) 'pinned Node 24.18.0 is unavailable'
    $nodeSummary = Invoke-Program $nodePath @('-e',"const fs=require('fs');const x=JSON.parse(fs.readFileSync('frontend/package-lock.json','utf8'));const r=x.packages[''];const p=JSON.parse(fs.readFileSync('frontend/package.json','utf8'));for(const s of ['dependencies','devDependencies']){const a=p[s]||{},b=r[s]||{};if(JSON.stringify(a)!==JSON.stringify(b))throw new Error(s+' manifest/lock root mismatch')}console.log(JSON.stringify({lockfileVersion:x.lockfileVersion,dependencies:r.dependencies||{},devDependencies:r.devDependencies||{}}));") $true
    $frontendLock = if ($nodeSummary.ExitCode -eq 0) { ($nodeSummary.Output -join '') | ConvertFrom-Json } else { $null }
    Assert-True ($nodeSummary.ExitCode -eq 0) "Pinned Node could not parse/verify frontend lock: $($nodeSummary.Output -join ' ')"
    if ($null -ne $frontend) {
        Assert-True ([string]$frontend.license -eq 'ISC') 'frontend license mismatch'
        Assert-True ([string]$frontend.packageManager -eq 'npm@11.16.0') 'frontend packageManager mismatch'
        Assert-True ([string]$frontend.engines.node -eq '24.18.0') 'frontend Node engine mismatch'
        Assert-True ([string]$frontend.engines.npm -eq '11.16.0') 'frontend npm engine mismatch'
        $frontendText = Get-Content -LiteralPath (Join-Path $repo 'frontend\package.json') -Raw -Encoding utf8
        Assert-True ($frontendText -notmatch 'react-window') 'react-window remains in active frontend manifest'
        Assert-True ($frontendText -notmatch '"zod"\s*:') 'conditional Zod dependency has no active browser consumer'
        Assert-True (Has-Property $frontend.devDependencies 'jsdom') 'jsdom test runtime is missing'
        foreach ($section in @($frontend.dependencies,$frontend.devDependencies)) {
            foreach ($property in $section.PSObject.Properties) { Assert-True ([string]$property.Value -match '^\d+\.\d+\.\d+([+-][0-9A-Za-z.-]+)?$') "frontend direct dependency is not exact: $($property.Name)=$($property.Value)" }
        }
    }
    if ($null -ne $frontendLock) {
        Assert-True ([int]$frontendLock.lockfileVersion -eq 3) 'frontend lockfileVersion must be 3'
        Assert-SameSet @($frontendLock.dependencies.PSObject.Properties.Name) @($frontend.dependencies.PSObject.Properties.Name) 'frontend runtime lock roots' $true
        Assert-SameSet @($frontendLock.devDependencies.PSObject.Properties.Name) @($frontend.devDependencies.PSObject.Properties.Name) 'frontend dev lock roots' $true
        Assert-True ($frontendLockText -notmatch 'react-window') 'react-window remains in active frontend lock'
    }
}
function Get-TomlPackageValue([string]$Block, [string]$Name, [bool]$Required) {
    $match = [regex]::Match($Block, "(?m)^$([regex]::Escape($Name))\s*=\s*`"([^`"\x5c]*)`"\s*$")
    if ($match.Success) { return $match.Groups[1].Value }
    if ($Required) { Add-Failure "Cargo.lock package block lacks $Name" }
    return ''
}
function Get-CargoLockRecords {
    $path = Join-Path $repo 'Cargo.lock'
    $text = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
    Assert-True ($text -match '(?m)^version\s*=\s*4\s*$') 'Cargo.lock format must be version 4'
    $records = [System.Collections.Generic.List[object]]::new()
    $blocks = [regex]::Split($text, '(?m)^\[\[package\]\]\s*$')
    for ($index = 1; $index -lt $blocks.Count; $index++) {
        $name = Get-TomlPackageValue $blocks[$index] 'name' $true
        $version = Get-TomlPackageValue $blocks[$index] 'version' $true
        $source = Get-TomlPackageValue $blocks[$index] 'source' $false
        $checksum = Get-TomlPackageValue $blocks[$index] 'checksum' $false
        $sourceKind = if ($source -eq '') { 'local' } elseif ($source -eq 'registry+https://github.com/rust-lang/crates.io-index') { 'crates.io' } else { 'INVALID' }
        Assert-True ($sourceKind -ne 'INVALID') "Cargo.lock has a non-approved source: $name@$version $source"
        if ($sourceKind -eq 'local') {
            Assert-True ($checksum -eq '') "local Cargo package has a checksum: $name@$version"
        } else {
            Assert-True ($checksum -match '^[0-9A-Fa-f]{64}$') "Cargo registry checksum invalid: $name@$version"
        }
        $lockKey = "cargo:$name@$version`:$sourceKind"
        $records.Add([pscustomobject][ordered]@{
            ecosystem='rust'; name=$name; version=$version; rawSource=$source; rawChecksum=$checksum
            sourceKind=$sourceKind; lock_key=$lockKey; canonical="rust|$lockKey"
        })
    }
    Assert-True ($records.Count -gt 1) 'Cargo.lock dependency closure is empty'
    Assert-SameSet @($records | ForEach-Object { [string]$_.canonical }) @($records | ForEach-Object { [string]$_.canonical }) 'Cargo.lock canonical identities' $true
    return $records.ToArray()
}
function Initialize-PinnedCargoEnvironment {
    $toolchainRoot = Join-Path $repo '.cache\p7-1-002\rustup\toolchains\1.97.0-x86_64-pc-windows-msvc'
    $env:CARGO_HOME = Join-Path $repo '.cache\p7-1-002\cargo'
    $env:RUSTUP_HOME = Join-Path $repo '.cache\p7-1-002\rustup'
    $env:CARGO_TARGET_DIR = Join-Path $repo '.cache\p7-1-002\target'
    $env:CARGO_NET_OFFLINE = 'true'
    $env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'
    $env:RUSTC = Join-Path $toolchainRoot 'bin\rustc.exe'
    $env:RUSTDOC = Join-Path $toolchainRoot 'bin\rustdoc.exe'
}
function Get-CargoMetadata([string]$Target = '') {
    Initialize-PinnedCargoEnvironment
    $cargoPath = Join-Path $repo $pinnedCargoRelative.Replace('/','\')
    $arguments = @('metadata','--locked','--offline','--all-features','--format-version','1')
    if ($Target -ne '') { $arguments += @('--filter-platform',$Target) }
    $result = Invoke-Program $cargoPath $arguments $true
    Assert-True ($result.ExitCode -eq 0) "cargo metadata failed for target '$Target': $($result.Output -join ' ')"
    if ($result.ExitCode -ne 0) { return $null }
    try { return (($result.Output -join "`n") | ConvertFrom-Json) }
    catch { Add-Failure "cargo metadata JSON is invalid for target '$Target': $($_.Exception.Message)"; return $null }
}
function Get-ReachableCargoIds([object]$Metadata) {
    $nodes = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach ($node in @($Metadata.resolve.nodes)) { $nodes.Add([string]$node.id, $node) }
    $reachable = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $queue = [System.Collections.Generic.Queue[string]]::new()
    foreach ($root in @($Metadata.workspace_members)) {
        $id = [string]$root
        Assert-True ($nodes.ContainsKey($id)) "cargo metadata workspace member lacks a resolve node: $id"
        if ($nodes.ContainsKey($id) -and $reachable.Add($id)) { $queue.Enqueue($id) }
    }
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        foreach ($dependency in @($nodes[$id].deps)) {
            $child = [string]$dependency.pkg
            if ($reachable.Add($child)) { $queue.Enqueue($child) }
        }
    }
    return @($reachable)
}
function Get-NpmLockRecords {
    $nodePath = Join-Path $repo $pinnedNodeRelative.Replace('/','\')
    $javascript = @'
const fs=require('fs');
const lock=JSON.parse(fs.readFileSync('frontend/package-lock.json','utf8'));
const manifest=JSON.parse(fs.readFileSync('frontend/package.json','utf8'));
if(lock.lockfileVersion!==3||!lock.packages||Array.isArray(lock.packages))throw new Error('lockfile v3 packages object required');
const root=lock.packages['']; if(!root)throw new Error('root lock entry missing');
const sections=['dependencies','devDependencies','optionalDependencies'];
for(const s of sections){const a=manifest[s]||{},b=root[s]||{};const ak=Object.keys(a).sort(),bk=Object.keys(b).sort();if(JSON.stringify(ak)!==JSON.stringify(bk))throw new Error(`${s} root names mismatch`);for(const k of ak)if(a[k]!==b[k])throw new Error(`${s} root spec mismatch ${k}`)}
if(root.name!==manifest.name||root.version!==manifest.version||root.license!==manifest.license)throw new Error('root package metadata mismatch');
const direct=new Set(sections.flatMap(s=>Object.keys(root[s]||{})));
const rows=Object.keys(lock.packages).sort().map(k=>{const e=lock.packages[k];if(k==='')return{lockKey:'.',name:e.name,version:e.version,scope:'runtime',direct:true,source:'repository:frontend/package.json',checksum:'NOT_APPLICABLE_LOCAL',metadataUrl:'repository:frontend/package.json',license:manifest.license,integrityAlgorithm:'',integrityHex:''};if(e.link)throw new Error(`linked entry ${k}`);const marker='node_modules/',pos=k.lastIndexOf(marker);if(pos<0)throw new Error(`invalid lock key ${k}`);const name=e.name||k.slice(pos+marker.length);if(typeof e.version!=='string'||!e.version)throw new Error(`missing version ${k}`);if(typeof e.resolved!=='string'||!e.resolved.startsWith('https://registry.npmjs.org/'))throw new Error(`non-official resolved ${k}`);const u=new URL(e.resolved);if(u.username||u.password||u.search||u.hash||u.hostname!=='registry.npmjs.org')throw new Error(`unsafe resolved ${k}`);if(typeof e.integrity!=='string'||!e.integrity)throw new Error(`missing integrity ${k}`);const token=e.integrity.trim().split(/\s+/)[0],dash=token.indexOf('-'),alg=token.slice(0,dash).toLowerCase(),bytes=Buffer.from(token.slice(dash+1),'base64'),sizes={sha256:32,sha384:48,sha512:64};if(!sizes[alg]||bytes.length!==sizes[alg])throw new Error(`bad integrity ${k}`);return{lockKey:k,name,version:e.version,scope:e.dev===true?'dev':'runtime',direct:k===`node_modules/${name}`&&direct.has(name),source:'registry:npmjs',checksum:e.integrity,metadataUrl:`https://registry.npmjs.org/${encodeURIComponent(name)}/${encodeURIComponent(e.version)}`,license:null,integrityAlgorithm:alg,integrityHex:bytes.toString('hex').toUpperCase()}});
process.stdout.write(JSON.stringify({rows}));
'@
    $result = Invoke-Program $nodePath @('-e',$javascript) $true
    Assert-True ($result.ExitCode -eq 0) "pinned Node failed to normalize npm lock: $($result.Output -join ' ')"
    if ($result.ExitCode -ne 0) { return @() }
    try { $parsed = (($result.Output -join '') | ConvertFrom-Json) }
    catch { Add-Failure "normalized npm lock JSON is invalid: $($_.Exception.Message)"; return @() }
    $records = [System.Collections.Generic.List[object]]::new()
    foreach ($row in @($parsed.rows)) {
        $lockKey = "npm:$([string]$row.lockKey)"
        $records.Add([pscustomobject][ordered]@{
            ecosystem='npm'; name=[string]$row.name; version=[string]$row.version; scope=[string]$row.scope
            direct=([bool]$row.direct).ToString().ToLowerInvariant(); source=[string]$row.source; checksum=[string]$row.checksum
            metadata_url=[string]$row.metadataUrl; licenseExpression=$(if ($null -eq $row.license) { '' } else { [string]$row.license })
            integrityAlgorithm=[string]$row.integrityAlgorithm; integrityHex=[string]$row.integrityHex
            lock_key=$lockKey; canonical="npm|$lockKey"
        })
    }
    Assert-True ($records.Count -gt 1) 'npm lock dependency closure is empty'
    Assert-SameSet @($records | ForEach-Object { [string]$_.canonical }) @($records | ForEach-Object { [string]$_.canonical }) 'npm lock canonical identities' $true
    return $records.ToArray()
}
function Get-ExpectedComponentUniverse {
    if ($null -ne $script:ExpectedComponents) { return $script:ExpectedComponents }
    $cargoLocks = @(Get-CargoLockRecords)
    $metadata = Get-CargoMetadata
    $records = [System.Collections.Generic.List[object]]::new()
    if ($null -ne $metadata) {
        Assert-JsonInteger $metadata.version 1 'cargo metadata format version'
        Assert-True (@($metadata.workspace_members).Count -eq 1) 'Cargo workspace must have exactly one resolution member'
        $packagesByKey = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
        $packagesById = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
        foreach ($package in @($metadata.packages)) {
            $source = if ($null -eq $package.source) { '' } else { [string]$package.source }
            $key = "$([string]$package.name)|$([string]$package.version)|$source"
            Assert-True (-not $packagesByKey.ContainsKey($key)) "duplicate cargo metadata package: $key"
            if (-not $packagesByKey.ContainsKey($key)) { $packagesByKey.Add($key,$package) }
            if (-not $packagesById.ContainsKey([string]$package.id)) { $packagesById.Add([string]$package.id,$package) }
        }
        $nodes = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
        foreach ($node in @($metadata.resolve.nodes)) { $nodes[[string]$node.id] = $node }
        $runtimeReach = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        $devReach = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        $directIds = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        $runtimeQueue = [System.Collections.Generic.Queue[string]]::new()
        $devQueue = [System.Collections.Generic.Queue[string]]::new()
        foreach ($root in @($metadata.workspace_members)) {
            $rootId = [string]$root
            [void]$runtimeReach.Add($rootId); [void]$directIds.Add($rootId); $runtimeQueue.Enqueue($rootId)
            foreach ($dependency in @($nodes[$rootId].deps)) {
                [void]$directIds.Add([string]$dependency.pkg)
                if (@($dependency.dep_kinds | Where-Object { [string]$_.kind -eq 'dev' }).Count -gt 0 -and $devReach.Add([string]$dependency.pkg)) { $devQueue.Enqueue([string]$dependency.pkg) }
            }
        }
        while ($runtimeQueue.Count -gt 0) {
            $id = $runtimeQueue.Dequeue()
            foreach ($dependency in @($nodes[$id].deps)) {
                $kinds = @($dependency.dep_kinds)
                $nonDev = $kinds.Count -eq 0 -or @($kinds | Where-Object { [string]$_.kind -ne 'dev' }).Count -gt 0
                if ($nonDev -and $runtimeReach.Add([string]$dependency.pkg)) { $runtimeQueue.Enqueue([string]$dependency.pkg) }
            }
        }
        while ($devQueue.Count -gt 0) {
            $id = $devQueue.Dequeue()
            foreach ($dependency in @($nodes[$id].deps)) {
                $kinds = @($dependency.dep_kinds)
                $nonDev = $kinds.Count -eq 0 -or @($kinds | Where-Object { [string]$_.kind -ne 'dev' }).Count -gt 0
                if ($nonDev -and $devReach.Add([string]$dependency.pkg)) { $devQueue.Enqueue([string]$dependency.pkg) }
            }
        }
        Assert-True ($cargoLocks.Count -eq $packagesByKey.Count) 'Cargo.lock/cargo metadata package count mismatch'
        foreach ($lock in $cargoLocks) {
            $metadataKey = "$($lock.name)|$($lock.version)|$($lock.rawSource)"
            Assert-True ($packagesByKey.ContainsKey($metadataKey)) "Cargo.lock package missing from metadata: $metadataKey"
            if (-not $packagesByKey.ContainsKey($metadataKey)) { continue }
            $package = $packagesByKey[$metadataKey]
            $id = [string]$package.id
            $scope = if ($runtimeReach.Contains($id)) { 'runtime' } elseif ($devReach.Contains($id)) { 'dev' } else { '' }
            Assert-True ($scope -ne '') "Cargo package is unreachable from runtime/dev graph: $metadataKey"
            $isLocal = [string]$lock.sourceKind -eq 'local'
            if ($isLocal) {
                Assert-True ([string]$lock.name -eq 'rbcsp-dependency-baseline' -and [string]$lock.version -eq '0.0.0') 'unexpected local Cargo workspace package'
            }
            $records.Add([pscustomobject][ordered]@{
                ecosystem='rust'; name=[string]$lock.name; version=[string]$lock.version; scope=$scope
                direct=$directIds.Contains($id).ToString().ToLowerInvariant()
                source=$(if ($isLocal) { 'repository:crates/dependency-baseline/Cargo.toml' } else { [string]$lock.rawSource })
                checksum=$(if ($isLocal) { 'NOT_APPLICABLE_LOCAL' } else { ([string]$lock.rawChecksum).ToLowerInvariant() })
                metadata_url=$(if ($isLocal) { 'repository:crates/dependency-baseline/Cargo.toml' } else { "https://crates.io/api/v1/crates/$([Uri]::EscapeDataString([string]$lock.name))/$([Uri]::EscapeDataString([string]$lock.version))" })
                licenseExpression=$(if ($null -eq $package.license) { '' } else { [string]$package.license })
                integrityAlgorithm=$(if ($isLocal) { '' } else { 'sha256' }); integrityHex=$(if ($isLocal) { '' } else { ([string]$lock.rawChecksum).ToUpperInvariant() })
                lock_key=[string]$lock.lock_key; canonical=[string]$lock.canonical
            })
        }
        foreach ($target in $expectedTargets) {
            $targetMetadata = Get-CargoMetadata $target
            if ($null -eq $targetMetadata) { continue }
            $reachable = @(Get-ReachableCargoIds $targetMetadata)
            $targetPackagesById = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
            foreach ($package in @($targetMetadata.packages)) { $targetPackagesById[[string]$package.id] = $package }
            $excludedReachable = @($reachable | Where-Object {
                $targetPackagesById.ContainsKey([string]$_) -and [string]$targetPackagesById[[string]$_].name -eq 'webpki-root-certs' -and [string]$targetPackagesById[[string]$_].version -eq '1.0.8'
            })
            Assert-True ($excludedReachable.Count -eq 0) "platform-excluded webpki-root-certs@1.0.8 is reachable for $target"
        }
    }
    foreach ($npm in @(Get-NpmLockRecords)) { $records.Add($npm) }
    $canonical = @($records | ForEach-Object { [string]$_.canonical })
    Assert-SameSet $canonical $canonical 'combined lock canonical identities' $true
    Assert-True ($canonical -ccontains $platformExcludedKey) 'approved platform-excluded component is absent from Cargo.lock'
    $script:ExpectedComponents = $records.ToArray()
    return $script:ExpectedComponents
}
function Test-GraphAndRejectedEvidence {
    $graph = Read-Json (Join-Path $p7 'evidence\p7-1-002\dependency-graph-scope.json')
    if ($null -ne $graph) {
        Assert-True ([string]$graph.zeroConsumerScope -eq 'ACTIVE_P7_TARGET_GRAPH_ONLY') 'graph evidence zero-consumer scope mismatch'
        Assert-True (-not [bool]$graph.repositoryWideZeroConsumerClaim) 'graph evidence makes repository-wide claim'
        Assert-True ([string]$graph.legacyGraphDisposition -eq 'FROZEN_EXCLUDED_PENDING_OWNER_TASK') 'graph legacy disposition mismatch'
        Assert-True (-not [bool]$graph.activeGraph.rust.businessLogicAllowed) 'graph allows Rust business logic'
        Assert-True (-not [bool]$graph.excludedLegacyGraph.includedInActiveResolution -and -not [bool]$graph.excludedLegacyGraph.mayBeModifiedByP7_1_002) 'legacy graph is active or mutable'
        Assert-True ([int]$graph.embedSelection.requiredActiveCount -eq 1) 'embed selection count mismatch'
    }
    $rows = @(Import-Csv -LiteralPath (Join-Path $p7 'evidence\p7-1-002\rejected-dependency-consumers.tsv') -Delimiter "`t")
    $expectedIds = @('P6-D012','P6-D031','P6-D032','P6-D033','P6-D034','P6-D035','P6-D036','P6-D037','P6-D038','P6-D039','P6-D040','P6-D041','P6-D042')
    Assert-SameSet @($rows.risk_id) $expectedIds 'rejected dependency risk IDs'
    foreach ($row in $rows) {
        Assert-True ([string]$row.active_graph_status -eq 'ZERO_CONSUMER_REQUIRED') "rejected row active status mismatch: $($row.risk_id)"
        Assert-True ([string]$row.zero_consumer_scope -eq 'ACTIVE_P7_TARGET_GRAPH_ONLY') "rejected row scope mismatch: $($row.risk_id)"
        Assert-True ([string]$row.repository_wide_claim -eq 'FALSE') "rejected row makes repository-wide claim: $($row.risk_id)"
        Assert-True ([string]$row.legacy_graph_disposition -eq 'FROZEN_EXCLUDED_PENDING_OWNER_TASK') "rejected row legacy disposition mismatch: $($row.risk_id)"
        Assert-True ([string]$row.owner_task -match '^P7\.[1-4]-\d{3}$') "rejected row owner missing: $($row.risk_id)"
    }
}
function New-RowMap([object[]]$Rows, [string]$Label, [bool]$AllowRepeatedKeys = $false) {
    $map = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach ($row in @($Rows)) {
        $key = "$([string]$row.ecosystem)|$([string]$row.lock_key)"
        Assert-True ($key -match '^(rust\|cargo:|npm\|npm:)') "$Label invalid canonical key: $key"
        if ($map.ContainsKey($key)) {
            if (-not $AllowRepeatedKeys) { Add-Failure "$Label duplicate canonical key: $key" }
        } else { $map.Add($key,$row) }
    }
    return $map
}
function Get-SbomProperty([object]$Component, [string]$Name) {
    $matches = @($Component.properties | Where-Object { [StringComparer]::Ordinal.Equals([string]$_.name,$Name) })
    Assert-True ($matches.Count -eq 1) "SBOM property multiplicity for $Name on $($Component.name)"
    if ($matches.Count -eq 1) { return [string]$matches[0].value }
    return ''
}
function Test-GeneratedEvidence {
    $expected = @(Get-ExpectedComponentUniverse)
    $expectedMap = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach ($row in $expected) { $expectedMap.Add([string]$row.canonical,$row) }
    $expectedKeys = @($expectedMap.Keys)
    $locked = @(Read-ExactTsv 'project-governance/current/p7/evidence/p7-1-002/locked-components.tsv' @('ecosystem','name','version','scope','direct','source','checksum','lock_key'))
    $metadata = @(Read-ExactTsv 'project-governance/current/p7/evidence/p7-1-002/official-metadata-sources.tsv' @('ecosystem','name','version','metadata_url','license_expression','verification','lock_key'))
    $licenses = @(Read-ExactTsv 'project-governance/current/p7/evidence/p7-1-002/license-allowlist.tsv' @('ecosystem','name','version','license_expression','decision','scope','direct','lock_key','decision_reason','target_evidence'))
    $licenseHashes = @(Read-ExactTsv 'project-governance/current/p7/evidence/p7-1-002/license-file-hashes.tsv' @('ecosystem','name','version','license_file','sha256','lock_key','evidence_status','reason'))
    $lockedMap = New-RowMap $locked 'locked-components'
    $metadataMap = New-RowMap $metadata 'official-metadata'
    $licenseMap = New-RowMap $licenses 'license-decisions'
    $hashMap = New-RowMap $licenseHashes 'license-hashes' $true
    Assert-SameSet @($lockedMap.Keys) $expectedKeys 'lock/locked-components closure' $true
    Assert-SameSet @($metadataMap.Keys) $expectedKeys 'lock/official-metadata closure' $true
    Assert-SameSet @($licenseMap.Keys) $expectedKeys 'lock/license-decision closure' $true
    Assert-SameSet @($hashMap.Keys) $expectedKeys 'lock/license-hash coverage' $true
    $policyInputs = [System.Collections.Generic.List[object]]::new()
    $policyIdByCombo = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    $policyComboByKey = [System.Collections.Generic.Dictionary[string,string]]::new([StringComparer]::Ordinal)
    foreach ($policyKey in @($expectedKeys | Where-Object { $_ -cne $platformExcludedKey })) {
        $expectedRow = $expectedMap[[string]$policyKey]
        $licenseRow = $licenseMap[[string]$policyKey]
        $combo = @([string]$expectedRow.ecosystem,[string]$expectedRow.scope,[string]$licenseRow.license_expression) -join [char]31
        if (-not $policyIdByCombo.ContainsKey($combo)) {
            $policyId = "p$($policyInputs.Count)"
            $policyIdByCombo.Add($combo,$policyId)
            $policyInputs.Add([pscustomobject][ordered]@{
                id = $policyId
                ecosystem = [string]$expectedRow.ecosystem
                scope = [string]$expectedRow.scope
                expression = [string]$licenseRow.license_expression
            })
        }
        $policyComboByKey.Add([string]$policyKey,$combo)
    }
    $policyPayload = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($policyInputs | ConvertTo-Json -Compress -Depth 5)))
    $spdxParser = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\runtime\node-v24.18.0-win-x64\node_modules\npm\node_modules\spdx-expression-parse'
    $policyScript = @'
const parse=require(process.argv[1]);
const rows=JSON.parse(Buffer.from(process.argv[2],'base64').toString('utf8'));
const allowed={rust:new Set(['0BSD','Apache-2.0','BSD-2-Clause','BSD-3-Clause','CC0-1.0','ISC','MIT','MIT-0','Unicode-3.0','Unlicense','Zlib']),npm:new Set(['0BSD','Apache-2.0','BlueOak-1.0.0','BSD-2-Clause','BSD-3-Clause','CC0-1.0','ISC','MIT','MIT-0','MPL-2.0'])};
function ok(node,set,scope){if(node.license){return set.has(node.license)&&!node.exception&&(!new Set(['MPL-2.0','CC-BY-4.0']).has(node.license)||scope==='dev');}if(node.conjunction==='and')return ok(node.left,set,scope)&&ok(node.right,set,scope);if(node.conjunction==='or')return ok(node.left,set,scope)||ok(node.right,set,scope);return false;}
const out=rows.map(row=>{try{return {id:row.id,approved:ok(parse(row.expression),allowed[row.ecosystem],row.scope)};}catch{return {id:row.id,approved:false};}});
process.stdout.write(JSON.stringify(out));
'@
    $nodePath = Join-Path $repo $pinnedNodeRelative.Replace('/','\')
    $policyRun = Invoke-Program $nodePath @('-e',$policyScript,$spdxParser,$policyPayload) $true
    Assert-True ($policyRun.ExitCode -eq 0) 'SPDX policy evaluator failed'
    $policyApprovalById = [System.Collections.Generic.Dictionary[string,bool]]::new([StringComparer]::Ordinal)
    if ($policyRun.ExitCode -eq 0) {
        try {
            $policyResults = (($policyRun.Output -join '') | ConvertFrom-Json)
            foreach ($result in @($policyResults)) { $policyApprovalById.Add([string]$result.id,[bool]$result.approved) }
        } catch { Add-Failure "SPDX policy evaluator returned invalid JSON: $($_.Exception.Message)" }
    }
    $platformCount = 0
    foreach ($key in $expectedKeys) {
        $want = $expectedMap[$key]; $have = $lockedMap[$key]; $meta = $metadataMap[$key]; $license = $licenseMap[$key]
        foreach ($field in @('ecosystem','name','version','scope','direct','source','checksum','lock_key')) { Assert-True ([StringComparer]::Ordinal.Equals([string]$have.$field,[string]$want.$field)) "locked field mismatch $key/$field" }
        foreach ($row in @($meta,$license)) {
            foreach ($field in @('ecosystem','name','version','lock_key')) { Assert-True ([StringComparer]::Ordinal.Equals([string]$row.$field,[string]$want.$field)) "evidence identity mismatch $key/$field" }
        }
        Assert-True ([string]$meta.verification -eq 'PASS') "metadata verification failed: $key"
        Assert-True ([string]$meta.metadata_url -eq [string]$want.metadata_url) "metadata URL mismatch: $key"
        Assert-True ([string]$meta.license_expression -eq [string]$license.license_expression) "metadata/license expression mismatch: $key"
        Assert-True ([string]$license.scope -eq [string]$want.scope -and [string]$license.direct -eq [string]$want.direct) "license scope/direct mismatch: $key"
        Assert-True (-not [string]::IsNullOrWhiteSpace([string]$license.license_expression)) "empty license expression: $key"
        if ($key -eq $platformExcludedKey) {
            $platformCount++
            Assert-True ([string]$license.license_expression -eq 'CDLA-Permissive-2.0') 'platform exclusion license expression mismatch'
            Assert-True ([string]$license.decision -eq 'PLATFORM_EXCLUDED') 'platform exclusion decision mismatch'
            Assert-True ([string]$license.decision_reason -eq 'UNREACHABLE_FROM_X86_64_PC_WINDOWS_MSVC_AND_X86_64_UNKNOWN_LINUX_GNU_ACTIVE_TARGET_GRAPHS') 'platform exclusion reason mismatch'
            Assert-True ([string]$license.target_evidence -eq 'ABSENT_CARGO_ID_FROM_CARGO_METADATA_FILTER_PLATFORM:x86_64-pc-windows-msvc,x86_64-unknown-linux-gnu') 'platform exclusion target evidence mismatch'
        } else {
            Assert-True ([string]$license.decision -eq 'APPROVED') "non-approved license decision: $key"
            Assert-True ([string]$license.target_evidence -eq 'NOT_APPLICABLE') "unexpected target evidence: $key"
            $policyId = $policyIdByCombo[$policyComboByKey[$key]]
            Assert-True ($policyApprovalById.ContainsKey($policyId) -and $policyApprovalById[$policyId]) "SPDX expression does not satisfy the approved policy: $key"
            if ([string]$license.license_expression -match '(^|\W)MPL-2\.0($|\W)') {
                Assert-True ([string]$want.ecosystem -eq 'npm' -and [string]$want.scope -eq 'dev') "MPL-2.0 is not npm dev-only: $key"
                Assert-True ([string]$license.decision_reason -eq 'DEV_ONLY_UNMODIFIED_EXCLUDED_FROM_FINAL_RUNTIME_BUNDLES') "MPL-2.0 reason mismatch: $key"
            } else { Assert-True ([string]$license.decision_reason -eq 'SPDX_POLICY_SATISFIED') "license approval reason mismatch: $key" }
            Assert-True ([string]$license.license_expression -notmatch '(^|\W)CC-BY-4\.0($|\W)') "CC-BY-4.0 cannot enter the runtime/dev lock graph: $key"
        }
    }
    Assert-True ($platformCount -eq 1) 'PLATFORM_EXCLUDED must occur exactly once'
    $hashPairs = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($row in $licenseHashes) {
        $key = "$([string]$row.ecosystem)|$([string]$row.lock_key)"
        Assert-True ($expectedMap.ContainsKey($key)) "license hash has unknown key: $key"
        if ($expectedMap.ContainsKey($key)) { foreach ($field in @('ecosystem','name','version','lock_key')) { Assert-True ([string]$row.$field -eq [string]$expectedMap[$key].$field) "license hash identity mismatch: $key/$field" } }
        Assert-True ($hashPairs.Add("$key|$([string]$row.license_file)")) "duplicate license-file row: $key/$($row.license_file)"
        if ([string]$row.license_file -eq 'NO_PACKAGED_LICENSE_FILE') {
            Assert-True ([string]$row.sha256 -eq 'NOT_APPLICABLE' -and [string]$row.evidence_status -eq 'ABSENT_WITH_EXPLICIT_DECISION' -and [string]$row.reason -eq 'NO_LICENSE_FILE_IN_VERIFIED_PACKAGE_ARCHIVE') "invalid absent-license sentinel: $key"
        } else {
            Assert-True (Test-SafeRepositoryRelativePath ([string]$row.license_file)) "unsafe license_file: $key/$($row.license_file)"
            Assert-True ([string]$row.sha256 -match '^[0-9A-F]{64}$' -and [string]$row.evidence_status -eq 'HASHED' -and [string]$row.reason -eq 'PACKAGED_LICENSE_FILE') "invalid packaged-license evidence: $key"
        }
    }
    foreach ($key in $expectedKeys) { Assert-True (@($licenseHashes | Where-Object { "$([string]$_.ecosystem)|$([string]$_.lock_key)" -ceq $key }).Count -gt 0) "license hash coverage missing: $key" }

    $sbom = Read-Json (Join-Path $p7 'evidence\p7-1-002\initial-sbom.cdx.json')
    $sbomKeys = [System.Collections.Generic.List[string]]::new()
    if ($null -ne $sbom) {
        Assert-ExactProperties $sbom @('bomFormat','specVersion','version','metadata','components') 'SBOM root'
        Assert-True ([string]$sbom.bomFormat -eq 'CycloneDX' -and [string]$sbom.specVersion -eq '1.6') 'SBOM format/spec mismatch'
        Assert-JsonInteger $sbom.version 1 'SBOM version'
        Assert-ExactProperties $sbom.metadata @('lifecycles','properties') 'SBOM metadata'
        Assert-True (@($sbom.metadata.lifecycles).Count -eq 1 -and [string]$sbom.metadata.lifecycles[0].phase -eq 'pre-build') 'SBOM lifecycle mismatch'
        $metaProps = @($sbom.metadata.properties | ForEach-Object { "$([string]$_.name)=$([string]$_.value)" })
        Assert-SameSet $metaProps @('rbcsp:p7:task=P7.1-002','rbcsp:p7:inventoryModel=ONE_COMPONENT_PER_LOCK_ENTRY','rbcsp:p7:zeroConsumerScope=ACTIVE_P7_TARGET_GRAPH_ONLY','rbcsp:p7:rustSbomCrossValidation=PASS_CARGO_CYCLONEDX_0.5.9') 'SBOM metadata properties' $true
        $bomRefs = [System.Collections.Generic.List[string]]::new()
        foreach ($component in @($sbom.components)) {
            $allowedProperties = @('type','bom-ref','name','version','scope','purl','licenses','properties')
            if (Has-Property $component 'hashes') { $allowedProperties += 'hashes' }
            if (Has-Property $component 'externalReferences') { $allowedProperties += 'externalReferences' }
            Assert-ExactProperties $component $allowedProperties "SBOM component $($component.name)"
            $ecosystem = Get-SbomProperty $component 'rbcsp:p7:ecosystem'
            $lockKey = Get-SbomProperty $component 'rbcsp:p7:lockKey'
            $key = "$ecosystem|$lockKey"; $sbomKeys.Add($key); $bomRefs.Add([string]$component.'bom-ref')
            Assert-True ($expectedMap.ContainsKey($key)) "SBOM component not in locks: $key"
            if ($expectedMap.ContainsKey($key)) {
                $want = $expectedMap[$key]
                Assert-True ([string]$component.name -eq [string]$want.name -and [string]$component.version -eq [string]$want.version) "SBOM identity mismatch: $key"
                Assert-True ((Get-SbomProperty $component 'rbcsp:p7:dependencyScope') -eq [string]$want.scope) "SBOM dependency scope mismatch: $key"
                Assert-True ((Get-SbomProperty $component 'rbcsp:p7:direct') -eq [string]$want.direct) "SBOM direct mismatch: $key"
                $decision = [string]$licenseMap[$key].decision
                Assert-True ((Get-SbomProperty $component 'rbcsp:p7:licenseDecision') -eq $decision) "SBOM license decision mismatch: $key"
                Assert-True (@($component.licenses).Count -eq 1 -and [string]$component.licenses[0].expression -eq [string]$licenseMap[$key].license_expression) "SBOM license mismatch: $key"
                $expectedScope = if ($decision -eq 'PLATFORM_EXCLUDED') { 'excluded' } elseif ([string]$want.scope -eq 'dev') { 'optional' } else { 'required' }
                Assert-True ([string]$component.scope -eq $expectedScope) "SBOM scope mismatch: $key"
            }
            Assert-True ([string]$component.'bom-ref' -match '^urn:rbcsp:p7-1-002:[0-9a-f]{64}$') "SBOM bom-ref invalid: $key"
            Assert-True ([string]$component.purl -match '^pkg:(cargo|npm)/') "SBOM purl invalid: $key"
            Assert-SameSet @($component.properties | ForEach-Object { [string]$_.name }) @('rbcsp:p7:ecosystem','rbcsp:p7:dependencyScope','rbcsp:p7:direct','rbcsp:p7:lockKey','rbcsp:p7:licenseDecision') "SBOM component property names: $key" $true
        }
        Assert-SameSet $sbomKeys.ToArray() $expectedKeys 'lock/SBOM closure' $true
        Assert-SameSet $bomRefs.ToArray() $bomRefs.ToArray() 'SBOM bom-ref uniqueness' $true
    }

    $cargoHash = Get-Sha256 (Join-Path $repo 'Cargo.lock'); $npmHash = Get-Sha256 (Join-Path $repo 'frontend\package-lock.json')
    $advisoryPath = Join-Path $p7 'evidence\p7-1-002\advisory-scan.json'; $advisory = Read-Json $advisoryPath
    if ($null -ne $advisory) {
        Assert-ExactProperties $advisory @('schemaVersion','recordId','taskId','capturedAt','state','networkUsed','rawCommandOutputPublished','commands','cargo','npm','lockHashes') 'advisory'
        Assert-JsonInteger $advisory.schemaVersion 1 'advisory schemaVersion'; Assert-True ([string]$advisory.recordId -eq 'P7-1-002-ADVISORY-2026-07-13-001') 'advisory recordId mismatch'
        Assert-True ([string]$advisory.taskId -eq 'P7.1-002' -and [string]$advisory.state -eq 'PASS' -and [string]$advisory.capturedAt -match '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$') 'advisory identity/time/state mismatch'
        Assert-JsonBoolean $advisory.networkUsed $true 'advisory networkUsed'; Assert-JsonBoolean $advisory.rawCommandOutputPublished $false 'advisory rawCommandOutputPublished'
        Assert-ExactProperties $advisory.lockHashes @('cargoSha256','frontendNpmSha256') 'advisory lockHashes'
        Assert-True ([string]$advisory.lockHashes.cargoSha256 -eq $cargoHash -and [string]$advisory.lockHashes.frontendNpmSha256 -eq $npmHash) 'advisory lock hash drift'
        Assert-ExactProperties $advisory.cargo @('toolVersion','vulnerabilities','advisories','bans','licenses','sources') 'advisory cargo'
        Assert-True ([string]$advisory.cargo.toolVersion -eq '0.20.2') 'advisory cargo tool mismatch'; Assert-JsonInteger $advisory.cargo.vulnerabilities 0 'cargo vulnerabilities'
        foreach ($field in @('advisories','bans','licenses','sources')) { Assert-True ([string]$advisory.cargo.$field -eq 'PASS') "cargo advisory field failed: $field" }
        Assert-ExactProperties $advisory.npm @('toolVersion','auditReportVersion','vulnerabilities','info','low','moderate','high','critical','total','dependencies') 'advisory npm'
        Assert-True ([string]$advisory.npm.toolVersion -eq '11.16.0') 'advisory npm tool mismatch'; Assert-JsonInteger $advisory.npm.auditReportVersion 2 'npm auditReportVersion'
        foreach ($field in @('vulnerabilities','info','low','moderate','high','critical','total')) { Assert-JsonInteger $advisory.npm.$field 0 "npm advisory $field" }
        Assert-JsonInteger $advisory.npm.dependencies (@($expected | Where-Object ecosystem -eq 'npm').Count - 1) 'npm advisory dependencies'
        Assert-SameSet @($advisory.commands | ForEach-Object { [string]$_.id }) @('cargo-deny-check','npm-audit-package-lock') 'advisory commands' $true
        foreach ($command in @($advisory.commands)) {
            Assert-ExactProperties $command @('id','binaryId','arguments','exitCode','normalizedResult') "advisory command $($command.id)"
            Assert-JsonInteger $command.exitCode 0 "advisory command exit $($command.id)"; Assert-True ([string]$command.normalizedResult -eq 'PASS') "advisory command result $($command.id)"
            if ([string]$command.id -eq 'cargo-deny-check') { Assert-True ([string]$command.binaryId -eq 'cargo-deny') 'cargo advisory binary mismatch'; Assert-SameSequence @($command.arguments) @('--all-features','check','advisories','bans','licenses','sources') 'cargo advisory arguments' }
            else { Assert-True ([string]$command.binaryId -eq 'npm') 'npm advisory binary mismatch'; Assert-SameSequence @($command.arguments) @('audit','--prefix','frontend','--package-lock-only','--json') 'npm advisory arguments' }
        }
    }

    $fingerprintPath = Join-Path $p7 'evidence\p7-1-002\toolchain-fingerprint.json'; $fingerprint = Read-Json $fingerprintPath
    if ($null -ne $fingerprint) {
        Assert-ExactProperties $fingerprint @('schemaVersion','recordId','taskId','capturedAt','state','pathsAreRepositoryRelative','rawCommandOutputPublished','nodeDistributionSha256','rust','node','npm','tools','binaries','commands') 'fingerprint'
        Assert-JsonInteger $fingerprint.schemaVersion 1 'fingerprint schemaVersion'; Assert-True ([string]$fingerprint.recordId -eq 'P7-1-002-TOOLCHAIN-FINGERPRINT-2026-07-13-001') 'fingerprint recordId mismatch'
        Assert-True ([string]$fingerprint.taskId -eq 'P7.1-002' -and [string]$fingerprint.state -eq 'PASS') 'fingerprint task/state mismatch'
        Assert-JsonBoolean $fingerprint.pathsAreRepositoryRelative $true 'fingerprint pathsAreRepositoryRelative'; Assert-JsonBoolean $fingerprint.rawCommandOutputPublished $false 'fingerprint rawCommandOutputPublished'
        Assert-True ([string]$fingerprint.nodeDistributionSha256 -eq '0AE68406B42D7725661DA979B1403EC9926DA205C6770827F33AAC9D8F26E821') 'fingerprint Node distribution hash mismatch'
        Assert-True ([string]$fingerprint.rust -eq '1.97.0' -and [string]$fingerprint.node -eq '24.18.0' -and [string]$fingerprint.npm -eq '11.16.0') 'fingerprint toolchain summary mismatch'
        Assert-ExactProperties $fingerprint.tools @('cargo-deny','cargo-about','cargo-cyclonedx') 'fingerprint tools'
        $contractBinaries = [System.Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal); foreach ($binary in @($script:Contract.fingerprintPolicy.requiredBinaries)) { $contractBinaries.Add([string]$binary.id,$binary) }
        Assert-SameSet @($fingerprint.binaries | ForEach-Object { [string]$_.id }) @($contractBinaries.Keys) 'fingerprint binaries' $true
        foreach ($binary in @($fingerprint.binaries)) {
            Assert-ExactProperties $binary @('id','path','version','sizeBytes','sha256') "fingerprint binary $($binary.id)"
            $id=[string]$binary.id; Assert-True ($contractBinaries.ContainsKey($id)) "unknown fingerprint binary: $id"; if (-not $contractBinaries.ContainsKey($id)) { continue }
            $required=$contractBinaries[$id]; foreach($field in @('path','version','sha256')){Assert-True ([string]$binary.$field -eq [string]$required.$field) "fingerprint binary mismatch: $id/$field"}
            Assert-True (Test-SafeRepositoryRelativePath ([string]$binary.path)) "unsafe fingerprint path: $id"
            $absolute=Join-Path $repo ([string]$binary.path).Replace('/','\'); Assert-True (Test-Path -LiteralPath $absolute -PathType Leaf) "fingerprint binary missing: $id"
            if(Test-Path -LiteralPath $absolute -PathType Leaf){Assert-JsonInteger $binary.sizeBytes (Get-Item -LiteralPath $absolute).Length "fingerprint binary size $id";Assert-True ((Get-Sha256 $absolute) -eq [string]$binary.sha256) "fingerprint binary hash mismatch: $id"}
        }
        $expectedCommands=[ordered]@{'rustc-version'=@('rustc','--version','1.97.0');'cargo-version'=@('cargo','--version','1.97.0');'node-version'=@('node','--version','24.18.0');'npm-version'=@('npm','--version','11.16.0');'cargo-deny-version'=@('cargo-deny','--version','0.20.2');'cargo-about-version'=@('cargo-about','--version','0.9.1');'cargo-cyclonedx-help'=@('cargo-cyclonedx','cyclonedx','--help','HELP_VERIFIED')}
        Assert-SameSet @($fingerprint.commands|ForEach-Object{[string]$_.id}) @($expectedCommands.Keys) 'fingerprint commands' $true
        foreach($command in @($fingerprint.commands)){Assert-ExactProperties $command @('id','binaryId','arguments','exitCode','normalizedResult') "fingerprint command $($command.id)";$definition=@($expectedCommands[[string]$command.id]);Assert-True ([string]$command.binaryId -eq $definition[0]) "fingerprint command binary mismatch: $($command.id)";Assert-SameSequence @($command.arguments) @($definition[1..($definition.Count-2)]) "fingerprint command arguments $($command.id)";Assert-JsonInteger $command.exitCode 0 "fingerprint command exit $($command.id)";Assert-True ([string]$command.normalizedResult -eq $definition[-1]) "fingerprint command result $($command.id)"}
    }
    $script:EvidenceState=[pscustomobject]@{expected=$expected;locked=$locked;metadata=$metadata;licenses=$licenses;licenseHashes=$licenseHashes;sbom=$sbom;advisory=$advisory;fingerprint=$fingerprint;cargoHash=$cargoHash;npmHash=$npmHash;advisoryHash=Get-Sha256 $advisoryPath;fingerprintHash=Get-Sha256 $fingerprintPath}
}
function Test-Completion {
    $completion=Read-Json $completionPath;if($null -eq $completion){return}
    Assert-ExactProperties $completion @('recordId','taskId','state','branch','expectedParent','commitAllowlist','protectedBaselineRows','zeroConsumerScope','repositoryWideZeroConsumerClaim','legacyGraphDisposition','lockHashes','componentCounts','advisory','toolchainFingerprint','qualityGates','negativeSideEffects','completionMarkdownSha256','actualCommitExcludedToAvoidSelfReference','commitEligible','nextTask','nextTaskBlockedUntil') 'completion'
    Assert-True ([string]$completion.recordId -eq 'P7-1-002-COMPLETION-2026-07-13-001' -and [string]$completion.taskId -eq 'P7.1-002' -and [string]$completion.state -eq 'P7_1_002_PASS_COMMIT_ELIGIBLE' -and [string]$completion.branch -eq $expectedBranch -and [string]$completion.expectedParent -eq $expectedParent) 'completion identity/state mismatch'
    Assert-True (@($completion.commitAllowlist).Count -eq 26) 'completion allowlist count mismatch';Assert-SameSet @($completion.commitAllowlist) $expectedAllowlist 'completion allowlist' $true;Assert-JsonInteger $completion.protectedBaselineRows 167 'completion protectedBaselineRows'
    Assert-True ([string]$completion.zeroConsumerScope -eq 'ACTIVE_P7_TARGET_GRAPH_ONLY' -and [string]$completion.legacyGraphDisposition -eq 'FROZEN_EXCLUDED_PENDING_OWNER_TASK') 'completion graph scope mismatch';Assert-JsonBoolean $completion.repositoryWideZeroConsumerClaim $false 'completion repositoryWideZeroConsumerClaim'
    Assert-ExactProperties $completion.lockHashes @('cargoSha256','frontendNpmSha256') 'completion lockHashes';Assert-True ([string]$completion.lockHashes.cargoSha256 -eq $script:EvidenceState.cargoHash -and [string]$completion.lockHashes.frontendNpmSha256 -eq $script:EvidenceState.npmHash) 'completion lock hashes mismatch'
    Assert-ExactProperties $completion.componentCounts @('cargo','npm','combined','lockedComponents','officialMetadata','licenseDecisions','licenseHashRows','licenseHashCoveredComponents','sbomComponents','platformExcluded') 'completion componentCounts'
    $cargoCount=@($script:EvidenceState.expected|Where-Object ecosystem -eq 'rust').Count;$npmCount=@($script:EvidenceState.expected|Where-Object ecosystem -eq 'npm').Count;$total=$cargoCount+$npmCount
    $counts=[ordered]@{cargo=$cargoCount;npm=$npmCount;combined=$total;lockedComponents=$script:EvidenceState.locked.Count;officialMetadata=$script:EvidenceState.metadata.Count;licenseDecisions=$script:EvidenceState.licenses.Count;licenseHashRows=$script:EvidenceState.licenseHashes.Count;licenseHashCoveredComponents=$total;sbomComponents=@($script:EvidenceState.sbom.components).Count;platformExcluded=1};foreach($field in $counts.Keys){Assert-JsonInteger $completion.componentCounts.$field $counts[$field] "completion componentCounts $field"}
    Assert-ExactProperties $completion.advisory @('state','cargoVulnerabilities','npmVulnerabilities','evidenceSha256') 'completion advisory';Assert-True ([string]$completion.advisory.state -eq 'PASS' -and [string]$completion.advisory.evidenceSha256 -eq $script:EvidenceState.advisoryHash) 'completion advisory binding mismatch';Assert-JsonInteger $completion.advisory.cargoVulnerabilities 0 'completion cargo vulnerabilities';Assert-JsonInteger $completion.advisory.npmVulnerabilities 0 'completion npm vulnerabilities'
    Assert-ExactProperties $completion.toolchainFingerprint @('state','binaries','commands','evidenceSha256') 'completion toolchainFingerprint';Assert-True ([string]$completion.toolchainFingerprint.state -eq 'PASS' -and [string]$completion.toolchainFingerprint.evidenceSha256 -eq $script:EvidenceState.fingerprintHash) 'completion fingerprint binding mismatch';Assert-JsonInteger $completion.toolchainFingerprint.binaries 7 'completion fingerprint binaries';Assert-JsonInteger $completion.toolchainFingerprint.commands 7 'completion fingerprint commands'
    $quality=@('cargo-check-locked-offline','cargo-test-locked-offline','cargo-clippy-locked-offline','cargo-fmt-check','cargo-deny-check','npm-ci-isolated','npm-ls-isolated','npm-dedupe-dry-run-isolated','evidence-builder-verify','evidence-builder-write');Assert-SameSet @($completion.qualityGates|ForEach-Object{[string]$_.id}) $quality 'completion quality gates' $true;foreach($gate in @($completion.qualityGates)){Assert-ExactProperties $gate @('id','exitCode','normalizedResult') "quality gate $($gate.id)";Assert-JsonInteger $gate.exitCode 0 "quality gate exit $($gate.id)";Assert-True ([string]$gate.normalizedResult -eq 'PASS') "quality gate failed: $($gate.id)"}
    Assert-ExactProperties $completion.negativeSideEffects @('rutgersRequests','vultrMutations','releasePublications','productionMutations') 'completion negativeSideEffects';foreach($field in @('rutgersRequests','vultrMutations','releasePublications','productionMutations')){Assert-JsonInteger $completion.negativeSideEffects.$field 0 "completion side effect $field"}
    $md=Join-Path $p7 'records\p7-1-002-completion.md';Assert-True ([string]$completion.completionMarkdownSha256 -eq (Get-Sha256 $md)) 'completion Markdown hash mismatch';Assert-JsonBoolean $completion.actualCommitExcludedToAvoidSelfReference $true 'completion actualCommitExcludedToAvoidSelfReference';Assert-JsonBoolean $completion.commitEligible $true 'completion commitEligible';Assert-True ([string]$completion.nextTask -eq 'P7.1-003' -and [string]$completion.nextTaskBlockedUntil -eq 'P7_1_002_PASS_POST_PUSH') 'completion next-task boundary mismatch'
}
function Test-PublicationText([string]$Content,[string]$Label){
    $escapedBackslash=[regex]::Escape([string][char]92)
    $pathSeparator='(?:/|'+$escapedBackslash+')'
    $uncPath='(?i)(^|[\s"''])'+$escapedBackslash+$escapedBackslash+'[^/\s'+$escapedBackslash+']+'+$pathSeparator+'[^\s"'']+'
    $windowsUserPath='(?i)(^|[\s"''])[A-Z]:'+$pathSeparator+'(?!'+$pathSeparator+')'
    $patterns=@(
        @{id='private-key';value='-----BEGIN [A-Z ]*PRIVATE KEY-----'},
        @{id='ssh-public-key';value='(?m)^ssh-(rsa|ed25519)\s+[A-Za-z0-9+/]{40,}'},
        @{id='aws-access-key';value='AKIA[0-9A-Z]{16}'},
        @{id='github-token';value='github_pat_[A-Za-z0-9_]{20,}|gh[pousr]_[A-Za-z0-9]{30,}'},
        @{id='npm-token';value='npm_[A-Za-z0-9]{20,}'},
        @{id='openai-token';value='sk-(proj-)?[A-Za-z0-9_-]{20,}'},
        @{id='slack-token';value='xox[baprs]-[A-Za-z0-9-]{20,}'},
        @{id='jwt';value='eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}'},
        @{id='authorization';value='(?i)(bearer|authorization:)\s+[A-Za-z0-9._~+/=-]{16,}'},
        @{id='credential-url';value='(?i)https?://[^/\s:@]+:[^/\s@]+@'},
        @{id='unc-path';value=$uncPath},
        @{id='windows-user-path';value=$windowsUserPath},
        @{id='posix-user-path';value='(?i)/(home|Users|mnt/[a-z])/[^/\s"'']+/'},
        @{id='email';value='(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}'}
    )
    foreach($entry in $patterns){
        Assert-True (-not [regex]::IsMatch($Content,[string]$entry.value)) "secret/PII/absolute-path rule $($entry.id) in $Label"
    }
}
function Test-PublicationContent([string]$Source){foreach($path in $expectedAllowlist){$spec=if($Source -eq 'index'){":$path"}else{"$Source`:$path"};$shown=Invoke-Git @('show',$spec) $true;Assert-True($shown.ExitCode -eq 0)"cannot read $Source task blob: $path";if($shown.ExitCode -eq 0){Test-PublicationText ($shown.Output -join "`n") "$Source blob $path"}}}
function Test-GitIgnoreDelta([string]$Source){$args=if($Source -eq 'index'){@('diff','--cached','--unified=0',$expectedParent,'--','.gitignore')}else{@('diff','--unified=0',$expectedParent,'HEAD','--','.gitignore')};$lines=@((Invoke-Git $args).Output);$added=@($lines|Where-Object{$_ -like '+*' -and $_ -notlike '+++*'}|ForEach-Object{([string]$_).Substring(1)});$removed=@($lines|Where-Object{$_ -like '-*' -and $_ -notlike '---*'});Assert-SameSequence $added @('/target/') '.gitignore approved addition';Assert-True($removed.Count -eq 0)'.gitignore has unapproved removals'}

Push-Location $repo
try{
    $branch=((Invoke-Git @('branch','--show-current')).Output -join '').Trim();Assert-True($branch -eq $expectedBranch)"active branch mismatch: $branch"
    Test-RequiredFileSet;Test-Contract;Test-PreviousTask;Test-ProtectedBaseline;Test-LicenseAndPins;Test-GraphAndRejectedEvidence;Test-GeneratedEvidence;Test-Completion
    $head=((Invoke-Git @('rev-parse','HEAD')).Output -join '').Trim();$remote=Invoke-Git @('rev-parse',"refs/remotes/origin/$expectedBranch") $true;$remoteHead=if($remote.ExitCode -eq 0){($remote.Output -join '').Trim()}else{''};Assert-True($remote.ExitCode -eq 0)'remote tracking ref is unavailable'
    if($Mode -eq 'PreCommit'){
        Assert-True($head -eq $expectedParent)"PreCommit HEAD mismatch: $head";Assert-True($remoteHead -eq $expectedParent)"PreCommit remote baseline mismatch: $remoteHead";$indexPaths=@(Get-IndexPaths);Assert-True($indexPaths.Count -eq 26)'staged path count is not 26';Assert-SameSet $indexPaths $expectedAllowlist 'staged task allowlist' $true
        $diffArgs=@('diff','--name-only','--')+$expectedAllowlist;$unstaged=@((Invoke-Git $diffArgs).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_ -ne ''});Assert-True($unstaged.Count -eq 0)"unstaged task changes exist: $($unstaged -join ',')";Test-GitIgnoreDelta 'index';Test-PublicationContent 'index'
    }else{
        $parent=((Invoke-Git @('rev-parse','HEAD^')).Output -join '').Trim();Assert-True($parent -eq $expectedParent)"$Mode commit parent mismatch: $parent";$paths=@(Get-CommitPaths 'HEAD');Assert-True($paths.Count -eq 26)"$Mode committed path count is not 26";Assert-SameSet $paths $expectedAllowlist "$Mode committed path allowlist" $true;Assert-True(@(Get-IndexPaths).Count -eq 0)"$Mode index is not empty"
        $diffArgs=@('diff','--name-only','--')+$expectedAllowlist;$unstaged=@((Invoke-Git $diffArgs).Output|ForEach-Object{Normalize-Path([string]$_)}|Where-Object{$_ -ne ''});Assert-True($unstaged.Count -eq 0)"$Mode has unstaged task changes";Test-GitIgnoreDelta 'HEAD';Test-PublicationContent 'HEAD';$message=((Invoke-Git @('log','-1','--format=%B','HEAD')).Output -join "`n");Test-PublicationText $message "$Mode commit message"
        if($Mode -eq 'PostCommit'){Assert-True($remoteHead -eq $expectedParent)'PostCommit remote advanced before validated push'}
        if($Mode -eq 'PostPush'){Assert-True($remoteHead -eq $head)"PostPush remote-tracking mismatch: $remoteHead";$upstream=Invoke-Git @('rev-parse','--abbrev-ref','--symbolic-full-name','@{upstream}') $true;Assert-True($upstream.ExitCode -eq 0 -and (($upstream.Output -join '').Trim() -eq "origin/$expectedBranch"))'PostPush upstream mismatch';$live=Invoke-Git @('ls-remote','--heads','origin',"refs/heads/$expectedBranch") $true;$liveOutput=@($live.Output);$liveHash=if($live.ExitCode -eq 0 -and $liveOutput.Count -eq 1){(([string]$liveOutput[0])-split '\s+')[0]}else{''};Assert-True($live.ExitCode -eq 0 -and $liveHash -eq $head)"PostPush live remote mismatch: $liveHash"}
    }
}finally{Pop-Location}
if ($failures.Count -gt 0) {
    Write-Output 'p7_1_002_gate=FAIL'
    $failures | ForEach-Object { Write-Output "failure=$_" }
    exit 1
}
$modeState = if ($Mode -eq 'PreCommit') { 'P7_1_002_PASS_PRE_COMMIT' } elseif ($Mode -eq 'PostCommit') { 'P7_1_002_PASS_POST_COMMIT' } else { 'P7_1_002_PASS_POST_PUSH' }
Write-Output "validation_mode=$Mode"
Write-Output "validation_state=$modeState"
Write-Output 'active_task=P7.1-002'
Write-Output "expected_parent=$expectedParent"
Write-Output 'task_allowlist_paths=26'
Write-Output 'protected_worktree_rows=167'
Write-Output 'dependency_scope=DEPENDENCY_RESOLUTION_SCAFFOLD_ONLY'
Write-Output 'zero_consumer_scope=ACTIVE_P7_TARGET_GRAPH_ONLY'
Write-Output 'repository_wide_zero_consumer_claim=FALSE'
Write-Output 'legacy_graph_disposition=FROZEN_EXCLUDED_PENDING_OWNER_TASK'
Write-Output 'license=ISC'
Write-Output 'locked_resolution=PASS'
Write-Output 'component_set_closure=PASS'
Write-Output 'official_metadata=PASS'
Write-Output 'license_allowlist=PASS'
Write-Output 'advisory_scan=PASS'
Write-Output 'toolchain_fingerprint=PASS'
Write-Output 'initial_sbom=PASS'
Write-Output 'secret_privacy_scan=PASS'
Write-Output 'rutgers_requests=0'
Write-Output 'vultr_mutations=0'
Write-Output 'release_publications=0'
Write-Output 'production_mutations=0'
Write-Output 'p7_1_002_gate=PASS'
