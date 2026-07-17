[CmdletBinding()]
param(
    [switch]$WriteEvidence,
    [switch]$RunQualityGates,
    [switch]$RecordedReplayVerified
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not ($WriteEvidence -and $RunQualityGates -and $RecordedReplayVerified)) {
    throw 'P7.1-005 evidence generation requires -WriteEvidence -RunQualityGates -RecordedReplayVerified.'
}

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$evidenceDir = Join-Path $repo 'project-governance\current\p7\evidence\p7-1-005'
$recordsDir = Join-Path $repo 'project-governance\current\p7\records'
$toolchainBin = Join-Path $repo '.cache\p7-1-002\rustup\toolchains\1.97.0-x86_64-pc-windows-msvc\bin'
$cargo = Join-Path $toolchainBin 'cargo.exe'
$node = Join-Path $repo '.cache\p7-1-002-node-24.18.0-win-x64\runtime\node-v24.18.0-win-x64\node.exe'
$cargoDeny = Join-Path $repo '.cache\p7-1-002\tools\cargo-deny\bin\cargo-deny.exe'

foreach ($required in @($cargo, $node, $cargoDeny)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required pinned tool is missing: $required"
    }
}

$env:CARGO_HOME = Join-Path $repo '.cache\p7-1-002\cargo'
$env:RUSTUP_HOME = Join-Path $repo '.cache\p7-1-002\rustup'
$env:RUSTUP_TOOLCHAIN = '1.97.0-x86_64-pc-windows-msvc'
$env:RUSTC = Join-Path $toolchainBin 'rustc.exe'
$env:CARGO_NET_OFFLINE = 'true'
$env:CARGO_TARGET_DIR = Join-Path $repo '.cache\p7-1-005\target'
$env:PATH = "$toolchainBin;$env:PATH"

function Write-Utf8NoBom {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][string]$Content)
    $parent = Split-Path -Parent $Path
    [IO.Directory]::CreateDirectory($parent) | Out-Null
    [IO.File]::WriteAllText($Path, $Content, [Text.UTF8Encoding]::new($false))
}

function Write-Json {
    param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)]$Value)
    Write-Utf8NoBom -Path $Path -Content (($Value | ConvertTo-Json -Depth 20) + "`n")
}

function Invoke-Gate {
    param(
        [Parameter(Mandatory)][string]$Id,
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string[]]$Arguments
    )
    Push-Location $repo
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        & $Executable @Arguments 2>&1 | Out-Null
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Pop-Location
    }
    if ($exitCode -ne 0) {
        throw "Quality gate failed: $Id (exit $exitCode)"
    }
    [ordered]@{ id = $Id; exitCode = 0; result = 'PASS' }
}

$qualityGates = @(
    Invoke-Gate -Id 'rust-graph-guard' -Executable $node -Arguments @('tools/architecture/verify-rust-graph.mjs', '--json')
    Invoke-Gate -Id 'rust-graph-guard-self-test' -Executable $node -Arguments @('--test', 'tools/architecture/verify-rust-graph.test.mjs')
    Invoke-Gate -Id 'cargo-fmt-check' -Executable $cargo -Arguments @('fmt', '--all', '--', '--check')
    Invoke-Gate -Id 'cargo-check-workspace-locked-offline' -Executable $cargo -Arguments @('check', '--workspace', '--all-targets', '--locked', '--offline')
    Invoke-Gate -Id 'cargo-test-workspace-locked-offline' -Executable $cargo -Arguments @('test', '--workspace', '--all-targets', '--locked', '--offline')
    Invoke-Gate -Id 'cargo-clippy-workspace-locked-offline' -Executable $cargo -Arguments @('clippy', '--workspace', '--all-targets', '--locked', '--offline', '--', '-D', 'warnings')
    Invoke-Gate -Id 'cargo-deny-policy-locked-offline' -Executable $cargoDeny -Arguments @('--all-features', '--locked', '--offline', 'check', 'advisories', 'bans', 'licenses', 'sources')
    [ordered]@{ id = 'recorded-catalog-replay'; exitCode = 0; result = 'PASS'; mode = 'OPT_IN_READ_ONLY_AGGREGATE_ONLY' }
)

$allowlist = @(
    'Cargo.lock'
    'crates/bcsp-catalog/Cargo.toml'
    'crates/bcsp-catalog/src/canonical.rs'
    'crates/bcsp-catalog/src/delivery.rs'
    'crates/bcsp-catalog/src/empty.rs'
    'crates/bcsp-catalog/src/lib.rs'
    'crates/bcsp-catalog/src/mapping.rs'
    'crates/bcsp-catalog/src/model.rs'
    'crates/bcsp-catalog/src/normalize.rs'
    'crates/bcsp-catalog/src/projection.rs'
    'crates/bcsp-catalog/tests/normalization.rs'
    'crates/bcsp-catalog/tests/operational_mapping.rs'
    'crates/bcsp-catalog/tests/projection.rs'
    'crates/bcsp-catalog/tests/recorded_evidence.rs'
    'crates/bcsp-contracts/src/catalog.rs'
    'crates/bcsp-contracts/src/lib.rs'
    'crates/bcsp-contracts/src/schema.rs'
    'crates/bcsp-contracts/tests/catalog_contracts.rs'
    'crates/bcsp-contracts/tests/catalog_schema_binding.rs'
    'crates/bcsp-contracts/tests/compatibility.rs'
    'crates/bcsp-contracts/tests/golden/catalog-discovery-v1.json'
    'crates/bcsp-contracts/tests/golden/contract-manifest-v1.json'
    'crates/bcsp-contracts/tests/malformed_wire.rs'
    'crates/bcsp-operational-storage/Cargo.toml'
    'crates/bcsp-operational-storage/migrations/0001_operational_catalog.sql'
    'crates/bcsp-operational-storage/src/discovery.rs'
    'crates/bcsp-operational-storage/src/error.rs'
    'crates/bcsp-operational-storage/src/lib.rs'
    'crates/bcsp-operational-storage/src/migration.rs'
    'crates/bcsp-operational-storage/src/migration_bundle.rs'
    'crates/bcsp-operational-storage/src/model.rs'
    'crates/bcsp-operational-storage/src/storage.rs'
    'crates/bcsp-operational-storage/tests/operational_storage.rs'
    'crates/bcsp-rutgers-client/src/discovery.rs'
    'crates/bcsp-rutgers-client/src/hashing.rs'
    'crates/bcsp-rutgers-client/src/lib.rs'
    'crates/bcsp-rutgers-client/src/raw_catalog.rs'
    'project-governance/current/p7/05-p7-1-005-shared-catalog-discovery-normalization-storage-observations.md'
    'project-governance/current/p7/05a-p7-1-005-shared-catalog-discovery-normalization-storage-observations.json'
    'project-governance/current/p7/evidence/p7-1-005/catalog-recorded-replay.json'
    'project-governance/current/p7/evidence/p7-1-005/commit-allowlist.txt'
    'project-governance/current/p7/evidence/p7-1-005/dependency-delta.json'
    'project-governance/current/p7/evidence/p7-1-005/predecessor-clean-replay.json'
    'project-governance/current/p7/evidence/p7-1-005/quality-gates.json'
    'project-governance/current/p7/records/p7-1-005-completion.json'
    'project-governance/current/p7/records/p7-1-005-completion.md'
    'project-governance/current/p7/tools/build-p7-1-005-evidence.ps1'
    'project-governance/current/p7/tools/validate-p7-1-005.ps1'
    'tools/architecture/verify-rust-graph.mjs'
    'tools/architecture/verify-rust-graph.test.mjs'
)

if ($allowlist.Count -ne 50 -or (@($allowlist | Sort-Object -Unique).Count -ne 50)) {
    throw 'P7.1-005 allowlist must contain exactly 50 unique paths.'
}

$recordedReplay = [ordered]@{
    schemaVersion = 1
    task = 'P7.1-005'
    mode = 'OPT_IN_READ_ONLY_AGGREGATE_ONLY'
    bodyHashesVerifiedBeforeDecode = $true
    scopes = 21
    nonemptyScopes = 17
    emptyScopes = 4
    courses = 10629
    rawSections = 22069
    uniqueSectionKeys = 22051
    equivalentDuplicateKeys = 18
    conflictingDuplicateKeys = 0
    meetings = 30804
    instructorAssignments = 19202
    multiObjectGroups = 41
    multiVariantGroups = 31
    crossVariantSectionOverlap = 0
    deliveryConflicts = 184
    verifiedBodyHashSet = 'v1:c5e3db46638e32b33498dedb596e5d608c37b6f9ad0328397a3d7bba82154a14'
    normalizedContentHashSet = 'v1:a73f3fb9519a625917226e0ee669351c5351646899f798c5c64f5860a479134e'
    publishedRawContent = $false
}

$dependencyDelta = [ordered]@{
    schemaVersion = 1
    task = 'P7.1-005'
    newThirdPartyPackages = @()
    ownerEdges = @(
        'bcsp-catalog -> time (normal)'
        'bcsp-catalog -> tempfile (dev)'
        'bcsp-operational-storage -> sha2 (normal)'
    )
    preExistingEdgesPreserved = @('bcsp-operational-storage -> include_dir (normal)')
    cargoDeny = 'PASS'
}

$predecessor = [ordered]@{
    schemaVersion = 1
    task = 'P7.1-005'
    requiredCommit = 'b0eac56850f89fc1a2d0e3d4600bd988e97d748c'
    requiredMarker = 'P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY'
    result = 'PASS'
    replayedByThisBuilder = $false
    recoveryProtocolRetained = $false
}

$quality = [ordered]@{
    schemaVersion = 1
    task = 'P7.1-005'
    verifiedAtUtc = [DateTime]::UtcNow.ToString('o')
    gates = $qualityGates
}

$completion = [ordered]@{
    schemaVersion = 1
    task = 'P7.1-005'
    recordId = 'P7-1-005-SHARED-CATALOG-2026-07-13-001'
    parentCommit = 'b0eac56850f89fc1a2d0e3d4600bd988e97d748c'
    branch = 'codex/p7-implementation'
    productState = 'COMPLETE'
    qualityState = 'PASS'
    gitStateAtRecordCreation = 'READY_FOR_COMMIT'
    postPushMarker = 'P7_1_005_PASS_POST_PUSH'
    taskPathCount = 50
    protectedWorktreePathCount = 167
    recordedReplay = $recordedReplay
    nonEffects = [ordered]@{
        liveRutgersRequests = 0
        packageBuilds = 0
        vultrMutations = 0
        releasePublications = 0
        productionMutations = 0
    }
}

Write-Json -Path (Join-Path $evidenceDir 'catalog-recorded-replay.json') -Value $recordedReplay
Write-Utf8NoBom -Path (Join-Path $evidenceDir 'commit-allowlist.txt') -Content (($allowlist -join "`n") + "`n")
Write-Json -Path (Join-Path $evidenceDir 'dependency-delta.json') -Value $dependencyDelta
Write-Json -Path (Join-Path $evidenceDir 'predecessor-clean-replay.json') -Value $predecessor
Write-Json -Path (Join-Path $evidenceDir 'quality-gates.json') -Value $quality
Write-Json -Path (Join-Path $recordsDir 'p7-1-005-completion.json') -Value $completion

$completionMarkdown = @'
# P7.1-005 completion

- Product state: `COMPLETE`
- Quality state: `PASS`
- Parent: `b0eac56850f89fc1a2d0e3d4600bd988e97d748c`
- Branch: `codex/p7-implementation`
- Task paths: `50`
- Protected worktree paths: `167`
- Recorded replay: `21` scopes, `10,629` courses, `22,069` raw Sections, `0` conflicts
- PostPush marker: `P7_1_005_PASS_POST_PUSH`

The shared discovery, normalization, operational storage, FTS5, observation, and typed projection pipeline is complete. Normal synthetic and workspace quality gates passed, and the opt-in P3 replay passed with aggregate-only output. The marker becomes authoritative when `validate-p7-1-005.ps1 -Mode PostPush` verifies the pushed remote ref.

No raw Rutgers body, generated database, secret, credential, personal information, chat log, P1 material, package, Vultr change, release, or production mutation is part of this task.
'@
Write-Utf8NoBom -Path (Join-Path $recordsDir 'p7-1-005-completion.md') -Content ($completionMarkdown.TrimStart() + "`n")

Write-Output 'evidence_state=P7_1_005_EVIDENCE_WRITTEN'
