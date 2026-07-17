param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path,
    [ValidateSet('Candidate','Final')]
    [string]$ValidationMode = 'Final'
)

$ErrorActionPreference = 'Stop'
$p5 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$p4 = (Resolve-Path (Join-Path $p5 '..\p4')).Path
$p3 = (Resolve-Path (Join-Path $p5 '..\p3')).Path
$failures = [System.Collections.Generic.List[string]]::new()
$storageDecisionId = 'P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001'

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { $failures.Add($Message) }
}

function Assert-SameSet([object[]]$Actual, [object[]]$Expected, [string]$Label) {
    $actualSet = @($Actual | ForEach-Object { [string]$_ } | Where-Object { $_ -ne '' } | Sort-Object -Unique)
    $expectedSet = @($Expected | ForEach-Object { [string]$_ } | Where-Object { $_ -ne '' } | Sort-Object -Unique)
    $difference = @(Compare-Object -ReferenceObject $expectedSet -DifferenceObject $actualSet)
    $detail = (($difference | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ',')
    Assert-True ($difference.Count -eq 0) "$Label set mismatch: $detail"
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Assert-PinnedHash([string]$BasePath, [string]$RelativePath, [string]$ExpectedHash, [string]$Label) {
    $path = Join-Path $BasePath $RelativePath
    Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "missing pinned $Label input: $RelativePath"
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        Assert-True ((Get-Sha256 $path) -eq $ExpectedHash) "pinned $Label hash drifted: $RelativePath"
    }
}

function Read-Text([string]$BasePath, [string]$RelativePath) {
    return Get-Content -LiteralPath (Join-Path $BasePath $RelativePath) -Raw -Encoding utf8
}

function Read-Json([string]$BasePath, [string]$RelativePath) {
    return Read-Text $BasePath $RelativePath | ConvertFrom-Json
}

function Get-Property([object]$Object, [string]$Name) {
    if ($null -eq $Object) { return $null }
    $property = @($Object.PSObject.Properties | Where-Object { $_.Name -eq $Name })
    if ($property.Count -eq 1) { return $property[0].Value }
    return $null
}

function Assert-PropertyEquals([object]$Object, [string]$Name, [object]$Expected, [string]$Label) {
    $actual = Get-Property $Object $Name
    Assert-True ($null -ne $actual) "$Label missing property $Name"
    if ($null -ne $actual) {
        Assert-True ([string]$actual -eq [string]$Expected) "$Label property $Name expected $Expected, got $actual"
    }
}

function Assert-Marker([string]$Text, [string]$Key, [string]$ExpectedValue, [string]$Document) {
    $pattern = '(?m)^\s*' + [regex]::Escape($Key) + '\s*=\s*' + [regex]::Escape($ExpectedValue) + '\s*$'
    Assert-True ([regex]::IsMatch($Text, $pattern)) "$Document missing machine marker $Key=$ExpectedValue"
}

function Assert-Headers([string]$Path, [string[]]$Expected, [string]$Label) {
    $firstLine = Get-Content -LiteralPath $Path -Encoding utf8 -TotalCount 1
    $actual = @([string]$firstLine -split "`t")
    Assert-SameSet $actual $Expected "$Label headers"
}

function Assert-ColumnsNonBlank([object[]]$Rows, [string[]]$Columns, [string]$Label) {
    foreach ($column in $Columns) {
        $blank = @($Rows | Where-Object { [string]::IsNullOrWhiteSpace([string]($_.$column)) })
        Assert-True ($blank.Count -eq 0) "$Label has $($blank.Count) blank value(s) in $column"
    }
}

function Split-IdentifierList([object]$Value) {
    if ($null -eq $Value) { return @() }
    return @(([string]$Value -split '[\s,;|]+') | Where-Object { $_ -ne '' })
}

Push-Location $RepositoryRoot
try {
    # P5 may consume only this byte-pinned P3/P4 baseline. Recomputing every
    # hash here prevents a later document edit from silently changing the
    # architecture while leaving the P5 prose and matrices untouched.
    $p3Pins = [ordered]@{
        '07-local-oneclick-implementation-plan.md' = '26B6805284E50639F91FD6523365FD8C0A9607353A0EF6B99D20B6E8910AA634'
        '09-p3-validation-and-freeze-gate.md' = '7CCB3CB067781961A17E7910D150325CC34B9390CEFEAE18E4F25A4E48CD9851'
        '23a-shared-open-final-contract.md' = '73031DD718A346D659214D6BE4C571D505836098DE42879E8861349CBD73887E'
        '23b-shared-open-final-contract.json' = '9BFE6712E959C39B6719E7A47F7F96E33AAC5BD20A8CFEF51320C0E2A0C77804'
    }
    $p4Pins = [ordered]@{
        '00-p4-charter-and-preflight.md' = 'B4E178CF1EBAE078250CEA1DAFA480169BD0A309BF72DFDFC75A2AD2B6A93647'
        '01-public-baseline-delta-matrix.tsv' = '70DE2D625C04E955E1947FFD2F6438E1B996340CCBA4994ADA2D633316CB5EB0'
        '02-public-runtime-session-capability-contract.md' = 'EAA63A7E4A58AB8C34BAE22215B3E1088ACABCD0065D8508B8B5607DF97DCB1B'
        '03-public-refresh-capacity-degradation-plan.md' = '9E63493058F53ECF0009B66E59714EE1C9E28DFCEAA7C3FF5CB4899185425DCD'
        '04-linux-package-and-operations-plan.md' = '71744604BA7C4CC7EC193A2CB8429BF7494412B9ED1613BA04E9C4AF2816CB90'
        '05-public-capability-deny-audit.tsv' = 'AB9F58D1B67A42924E09413120C67911BF0A5B6F75595FFB70A00C17947CA622'
        '06-p4-traceability-matrix.tsv' = 'F5E5BC153D5E1FF0F39FCE0BFAC373970D2960CE6352A939D5EBDC0B7EBA3AD1'
        '07-p4-validation-gate.md' = '3DF1C43EB15CDBDA4970A4D61E2B48AA474C4EC31E875001923B36D6B18BE650'
        '07a-p4-validation-gate.json' = 'A879B1988A2121344585C8B1D83992FDF5ADE805A0C3816722055A77D8EFEADD'
    }
    foreach ($entry in $p3Pins.GetEnumerator()) { Assert-PinnedHash $p3 $entry.Key $entry.Value 'P3' }
    foreach ($entry in $p4Pins.GetEnumerator()) { Assert-PinnedHash $p4 $entry.Key $entry.Value 'P4' }

    $p3LocalPlan = Read-Text $p3 '07-local-oneclick-implementation-plan.md'
    Assert-True ($p3LocalPlan -match 'P3_LOCAL_ONECLICK_PLAN_FROZEN') 'P3 local plan is not frozen'
    $p3Open = Read-Json $p3 '23b-shared-open-final-contract.json'
    Assert-PropertyEquals $p3Open 'status' 'FROZEN_P3_SHARED_OPEN_CONTRACT' 'P3 Open contract'
    Assert-True ([int]$p3Open.scheduler.publicGeneralSeconds -eq 30) 'P3 public general Open clock drifted'
    Assert-True ([int]$p3Open.scheduler.activeWatchTargetSeconds -eq 10) 'P3 active-watch clock drifted'
    Assert-True ([int]$p3Open.scheduler.localGeneralDefaultSeconds -eq 30) 'P3 local Open default drifted'
    Assert-True ([int]$p3Open.scheduler.localGeneralMinimumSeconds -eq 3 -and [int]$p3Open.scheduler.localGeneralMaximumSeconds -eq 3600) 'P3 local Open range drifted'

    $p4Gate = Read-Text $p4 '07-p4-validation-gate.md'
    $p4Record = Read-Json $p4 '07a-p4-validation-gate.json'
    Assert-Marker $p4Gate 'p4_gate' 'P4_PASS' 'P4 07 gate'
    Assert-Marker $p4Gate 'p5_eligible' 'TRUE' 'P4 07 gate'
    Assert-Marker $p4Gate 'p7_authorized' 'FALSE' 'P4 07 gate'
    Assert-PropertyEquals $p4Record 'p4Gate' 'P4_PASS' 'P4 validation record'
    Assert-True ([bool]$p4Record.eligibility.p5) 'P4 validation record does not authorize P5'
    Assert-True (-not [bool]$p4Record.eligibility.p7) 'P4 validation record authorizes P7'
    foreach ($property in @('p4RutgersRequestArtifacts','networkEvidenceRequests','productSourceMutations','productionMutations','packageBuilds')) {
        Assert-PropertyEquals $p4Record $property 0 'P4 validation record'
    }

    $requiredP5 = @(
        '00-p5-charter-and-preflight.md',
        '01-shared-local-public-capability-matrix.tsv',
        '02-adapter-entrypoint-config-boundaries.md',
        '03-ui-capability-and-build-exclusion-contract.md',
        '04-test-reuse-and-variant-matrix.tsv',
        '05-conflict-and-leakage-ledger.tsv',
        '06-p5-traceability-matrix.tsv',
        '07-p5-validation-gate.md',
        '07a-p5-validation-gate.json'
    )
    foreach ($name in $requiredP5) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p5 $name) -PathType Leaf) "missing P5 artifact: $name"
    }

    # P5 is architecture/governance only: no requests, evidence payloads,
    # product source, package, release or production-side artifacts are valid.
    $requestArtifactPattern = '(?i)(request[-_]?ledger|rutgers[-_]?(request|response)|raw[-_]?(body|response)|network[-_]?capture|http[-_]?capture|evidence[-_]?payload|\.har$|\.body$)'
    $requestArtifacts = @(Get-ChildItem -LiteralPath $p5 -Recurse -File | Where-Object { $_.FullName -match $requestArtifactPattern })
    Assert-True ($requestArtifacts.Count -eq 0) "P5 request/evidence artifacts must be zero: $($requestArtifacts.Name -join ',')"
    $allowedExtensions = @('.md','.tsv','.json','.ps1')
    $productArtifacts = @(Get-ChildItem -LiteralPath $p5 -Recurse -File | Where-Object { $_.Extension.ToLowerInvariant() -notin $allowedExtensions })
    Assert-True ($productArtifacts.Count -eq 0) "P5 product/package artifacts must be zero: $($productArtifacts.Name -join ',')"

    $charter = Read-Text $p5 '00-p5-charter-and-preflight.md'
    $boundaries = Read-Text $p5 '02-adapter-entrypoint-config-boundaries.md'
    $uiBoundary = Read-Text $p5 '03-ui-capability-and-build-exclusion-contract.md'

    $charterMarkers = [ordered]@{
        phase='P5'; status='P5_CORE_BOUNDARY_FROZEN'; p3_input_gate='P3_PASS'; p4_input_gate='P4_PASS';
        review_storage_amendment=$storageDecisionId;
        shared_domain_implementation_count='1'; shared_filter_schema_implementation_count='1';
        shared_open_implementation_count='1'; shared_react_ui_implementation_count='1'; long_lived_local_public_forks='0';
        final_package_count='2'; local_physical_database_count='1'; local_database_relative_path='data/rbcsp.sqlite';
        local_storage_path_anchor='RESOLVED_EXECUTABLE_ROOT'; local_storage_cwd_independent='TRUE';
        local_storage_fallback_allowed='FALSE'; public_state_root='/var/lib/bcsp'; public_personal_schema_linked='FALSE';
        release_archive_database_files='0'; release_archive_seed_or_real_data_files='0';
        rutgers_requests_authorized='0'; product_source_changes_authorized='0'; production_changes_authorized='0';
        p6_entry_requires_p5_validation='TRUE'; p7_authorized='FALSE'
    }
    foreach ($entry in $charterMarkers.GetEnumerator()) { Assert-Marker $charter $entry.Key $entry.Value 'P5 00 charter' }
    foreach ($entry in $p3Pins.GetEnumerator()) {
        Assert-True ($charter.Contains($entry.Value)) "P5 charter does not pin P3 hash $($entry.Key)"
    }
    foreach ($entry in $p4Pins.GetEnumerator()) {
        Assert-True ($charter.Contains($entry.Value)) "P5 charter does not pin P4 hash $($entry.Key)"
    }

    $boundaryMarkers = [ordered]@{
        status='P5_ADAPTER_ENTRYPOINT_CONFIG_BOUNDARIES_FROZEN'; review_storage_amendment=$storageDecisionId;
        rust_shared_domain_implementation_count='1';
        shared_open_implementation_count='1'; rust_binary_entrypoints='2'; local_binary='BCSP_LOCAL'; public_binary='BCSP_SERVER';
        long_lived_product_forks='0'; api_schema_version_divergence='0'; public_local_only_runtime_symbols='0';
        local_physical_database_count='1'; local_database_relative_path='data/rbcsp.sqlite';
        local_storage_path_anchor='RESOLVED_EXECUTABLE_ROOT'; local_storage_cwd_independent='TRUE';
        local_storage_fallback_allowed='FALSE'; local_operational_personal_logical_domains='2';
        public_state_root='/var/lib/bcsp'; public_personal_migrations_linked='FALSE'; public_personal_tables_linked='FALSE';
        release_archive_database_files='0'; release_archive_seed_or_real_data_files='0';
        public_build_uses_local_only_feature='FALSE'; cargo_feature_unification_leakage_allowed='FALSE';
        public_catalog_interval_sec='600'; public_open_general_interval_sec='30'; public_open_active_interval_sec='10';
        local_catalog_default_sec='600'; local_catalog_min_minutes='1'; local_catalog_max_minutes='1440';
        local_open_default_sec='30'; local_open_min_sec='3'; local_open_max_sec='3600';
        product_source_mutations='0'
    }
    foreach ($entry in $boundaryMarkers.GetEnumerator()) { Assert-Marker $boundaries $entry.Key $entry.Value 'P5 02 boundaries' }

    $uiMarkers = [ordered]@{
        status='P5_UI_BUILD_EXCLUSION_CONTRACT_FROZEN'; react_ui_implementation_count='1'; filter_schema_implementation_count='1';
        public_local_only_source_graph_nodes='0'; public_local_only_routes='0'; public_local_only_api_methods='0';
        public_local_only_storage_keys='0'; public_local_only_i18n_keys='0'; public_local_only_bundle_symbols='0';
        css_hiding_satisfies_exclusion='FALSE'; tree_shaking_only_satisfies_exclusion='FALSE';
        public_build_exclusion_requires_graph_proof='TRUE'; product_source_mutations='0'
    }
    foreach ($entry in $uiMarkers.GetEnumerator()) { Assert-Marker $uiBoundary $entry.Key $entry.Value 'P5 03 UI exclusion' }

    $boundaryCorpus = $charter + "`n" + $boundaries + "`n" + $uiBoundary
    foreach ($patternLabel in ([ordered]@{
        'entrypoint boundary'='(?i)(entrypoint|binary|入口)';
        'adapter boundary'='(?i)adapter';
        'config boundary'='(?i)config';
        'provider boundary'='(?i)provider';
        'package boundary'='(?i)package';
        'local persistence'='(?is)(local|本地).{0,100}(persist|持久)';
        'local Saved views'='(?is)(local|本地).{0,100}Saved views|Saved views.{0,100}(local|本地)';
        'local Reset'='(?is)(local|本地).{0,100}Reset|Reset.{0,100}(local|本地)';
        'single local package database'='(?is)data/rbcsp\.sqlite.{0,240}operational.{0,160}personal';
        'executable-root path anchor'='(?is)(RESOLVED_EXECUTABLE_ROOT|resolved-executable-root|<resolved-executable-root>).{0,160}(data/rbcsp\.sqlite|package root|package-root)';
        'CWD-independent no-fallback path'='(?is)CWD.{0,120}(fallback|回退)|(?:fallback|回退).{0,120}CWD';
        'public operational-only root'='(?is)/var/lib/bcsp.{0,120}(operational-only|operational store|operational DB|operational state)';
        'archive zero database payload'='(?is)(archive|release archive).{0,240}(WAL|SHM).{0,160}(seed|real)'
    }).GetEnumerator()) {
        Assert-True ([regex]::IsMatch($boundaryCorpus, $patternLabel.Value)) "P5 core documents do not establish $($patternLabel.Key)"
    }

    # The capability matrix must be a lossless one-to-one reclassification of
    # the 76 frozen P4 delta rows, not a hand-picked subset.
    $p4DeltaPath = Join-Path $p4 '01-public-baseline-delta-matrix.tsv'
    $p4Delta = @(Import-Csv -LiteralPath $p4DeltaPath -Delimiter "`t" -Encoding utf8)
    Assert-True ($p4Delta.Count -eq 76) "pinned P4 delta matrix expected 76 rows, got $($p4Delta.Count)"

    $capabilityPath = Join-Path $p5 '01-shared-local-public-capability-matrix.tsv'
    $capabilityHeaders = @('capability_id','source_refs','p4_delta_id','capability','classification','semantic_owner','module_boundary','local_consumer','public_consumer','build_edge','acceptance_test_id','p7_task_ids')
    Assert-Headers $capabilityPath $capabilityHeaders 'P5 capability matrix'
    $capabilities = @(Import-Csv -LiteralPath $capabilityPath -Delimiter "`t" -Encoding utf8)
    Assert-True ($capabilities.Count -eq 76) "P5 capability matrix expected 76 rows, got $($capabilities.Count)"
    Assert-ColumnsNonBlank $capabilities $capabilityHeaders 'P5 capability matrix'
    Assert-True (@($capabilities.capability_id | Sort-Object -Unique).Count -eq $capabilities.Count) 'P5 capability_id is not unique'
    Assert-True (@($capabilities.p4_delta_id | Sort-Object -Unique).Count -eq $capabilities.Count) 'P5 p4_delta_id is not unique'
    Assert-True (@($capabilities.acceptance_test_id | Sort-Object -Unique).Count -eq $capabilities.Count) 'P5 capability acceptance_test_id is not unique'
    $allowedClassifications = @('SHARED','LOCAL_ONLY','PUBLIC_ONLY','EXCLUDED')
    Assert-True (@($capabilities | Where-Object { $_.classification -notin $allowedClassifications }).Count -eq 0) 'P5 capability matrix contains an invalid classification'
    Assert-SameSet @($capabilities.classification) $allowedClassifications 'P5 capability classifications represented'
    Assert-True (@($capabilities | Where-Object { $_.classification -eq 'SHARED' }).Count -eq 46) 'P5 SHARED row count drifted'
    Assert-True (@($capabilities | Where-Object { $_.classification -eq 'LOCAL_ONLY' }).Count -eq 9) 'P5 LOCAL_ONLY row count drifted'
    Assert-True (@($capabilities | Where-Object { $_.classification -eq 'PUBLIC_ONLY' }).Count -eq 19) 'P5 PUBLIC_ONLY row count drifted'
    Assert-True (@($capabilities | Where-Object { $_.classification -eq 'EXCLUDED' }).Count -eq 2) 'P5 EXCLUDED row count drifted'
    Assert-SameSet @($capabilities.p4_delta_id) @($p4Delta.row_id) 'P5 coverage of all P4 delta rows'
    $capabilityById = @{}
    foreach ($row in $capabilities) { $capabilityById[[string]$row.capability_id] = $row }
    foreach ($id in @('P5-C057','P5-C064','P5-C065','P5-C072','P5-C074','P5-C076')) {
        Assert-True ($capabilityById.ContainsKey($id)) "P5 storage/package amendment capability row missing: $id"
        if ($capabilityById.ContainsKey($id)) {
            Assert-True ([string]$capabilityById[$id].source_refs -match [regex]::Escape($storageDecisionId)) "P5 capability $id does not cite $storageDecisionId"
        }
    }
    Assert-True (([string]$capabilityById['P5-C057'].capability) -match '(?i)/var/lib/bcsp.*operational-only.*zero personal') 'P5-C057 does not freeze public operational-only storage and zero personal schema'
    Assert-True (([string]$capabilityById['P5-C064'].capability) -match '(?i)RBCSP\.exe.*resolved-executable-root') 'P5-C064 does not freeze executable-root path anchoring'
    Assert-True (([string]$capabilityById['P5-C065'].capability) -match '(?i)personal table and migration domain.*data/rbcsp\.sqlite') 'P5-C065 does not freeze the local personal logical domain inside the single database'
    Assert-True (([string]$capabilityById['P5-C072'].capability) -match '(?i)zero DB WAL SHM seed or real data') 'P5-C072 does not freeze the Windows archive data denylist'
    Assert-True (([string]$capabilityById['P5-C074'].capability) -match '(?i)/var/lib/bcsp.*operational-only.*zero data payload') 'P5-C074 does not freeze Linux runtime storage and archive data boundaries'
    Assert-True (([string]$capabilityById['P5-C076'].capability) -match '(?i)prebuilt DB SQLite WAL SHM seed real Catalog Open') 'P5-C076 does not exclude prebuilt storage and real upstream data'
    $p4DispositionById = @{}
    foreach ($row in $p4Delta) { $p4DispositionById[[string]$row.row_id] = [string]$row.disposition }
    foreach ($row in $capabilities) {
        $allowedForDisposition = switch ($p4DispositionById[[string]$row.p4_delta_id]) {
            'INHERIT_SHARED' { @('SHARED') }
            'PUBLIC_MODIFY' { @('PUBLIC_ONLY') }
            'PUBLIC_EXCLUDE' { @('LOCAL_ONLY','EXCLUDED') }
            default { @() }
        }
        Assert-True ($row.classification -in $allowedForDisposition) "P5 classification $($row.classification) is incompatible with P4 disposition $($p4DispositionById[[string]$row.p4_delta_id]) for $($row.capability_id)"
    }

    $requiredFilters = @(
        'FLT-C01','FLT-C02','FLT-C03','FLT-C04','FLT-C05','FLT-C06','FLT-C07','FLT-C08','FLT-C09','FLT-C10',
        'FLT-S01','FLT-S02','FLT-S03','FLT-S04a','FLT-S04b','FLT-S05','FLT-S06','FLT-S07','FLT-S08','FLT-S09','FLT-S10','FLT-S11'
    )
    $capabilityCorpus = (($capabilities | ForEach-Object { "$($_.source_refs) $($_.capability) $($_.acceptance_test_id)" }) -join "`n")
    foreach ($filter in $requiredFilters) {
        $pattern = '(?<![A-Za-z0-9])' + [regex]::Escape($filter) + '(?![A-Za-z0-9])'
        Assert-True ([regex]::IsMatch($capabilityCorpus, $pattern)) "P5 capability matrix does not cover filter $filter"
        $filterRows = @($capabilities | Where-Object { ("$($_.source_refs) $($_.capability) $($_.acceptance_test_id)") -match $pattern })
        Assert-True (@($filterRows | Where-Object { $_.classification -ne 'SHARED' }).Count -eq 0) "P5 filter $filter is not owned by the one shared FilterSchema"
    }
    foreach ($localCapability in @('SAVED[_ -]?VIEWS?','RESET')) {
        $matching = @($capabilities | Where-Object { ("$($_.capability) $($_.source_refs)") -match "(?i)$localCapability" })
        Assert-True ($matching.Count -gt 0) "P5 capability matrix does not include local capability $localCapability"
        Assert-True (@($matching | Where-Object { $_.classification -ne 'LOCAL_ONLY' }).Count -eq 0) "P5 capability $localCapability is not exclusively LOCAL_ONLY"
    }
    $localPersistence = @($capabilities | Where-Object {
        $_.classification -eq 'LOCAL_ONLY' -and ("$($_.capability) $($_.source_refs)") -match '(?i)(PERSIST|PERSONAL|HISTORY)'
    })
    Assert-True ($localPersistence.Count -gt 0) 'P5 capability matrix does not retain LOCAL_ONLY persistence/history'

    # Every capability has the exact test family its classification requires.
    $testPath = Join-Path $p5 '04-test-reuse-and-variant-matrix.tsv'
    $testHeaders = @('test_row_id','capability_id','classification','test_kind','test_id','local_expectation','public_expectation','build_assertion','p7_task_ids','verification_gate')
    Assert-Headers $testPath $testHeaders 'P5 test matrix'
    $tests = @(Import-Csv -LiteralPath $testPath -Delimiter "`t" -Encoding utf8)
    Assert-True ($tests.Count -eq 106) "P5 test matrix expected 106 rows, got $($tests.Count)"
    Assert-ColumnsNonBlank $tests $testHeaders 'P5 test matrix'
    Assert-True (@($tests.test_row_id | Sort-Object -Unique).Count -eq $tests.Count) 'P5 test_row_id is not unique'
    Assert-True (@($tests.test_id | Sort-Object -Unique).Count -eq $tests.Count) 'P5 test_id is not unique'
    Assert-True (@($tests | Where-Object { $_.capability_id -notin $capabilities.capability_id }).Count -eq 0) 'P5 test matrix references an unknown capability_id'
    foreach ($capability in $capabilities) {
        $rows = @($tests | Where-Object { $_.capability_id -eq $capability.capability_id })
        Assert-True (@($rows | Where-Object { $_.classification -ne $capability.classification }).Count -eq 0) "P5 test classification drift for $($capability.capability_id)"
        $expectedKinds = switch ($capability.classification) {
            'SHARED' { @('SHARED_CONTRACT') }
            'LOCAL_ONLY' { @('LOCAL_VARIANT','PUBLIC_NEGATIVE_BUNDLE') }
            'PUBLIC_ONLY' { @('PUBLIC_VARIANT','LOCAL_NEGATIVE_BUNDLE') }
            'EXCLUDED' { @('LOCAL_NEGATIVE_BUNDLE','PUBLIC_NEGATIVE_BUNDLE') }
        }
        Assert-SameSet @($rows.test_kind) $expectedKinds "P5 test kinds for $($capability.capability_id)"
        Assert-True ($rows.Count -eq $expectedKinds.Count) "P5 test multiplicity drift for $($capability.capability_id)"
    }
    $testById = @{}
    foreach ($row in $tests) { $testById[[string]$row.test_id] = $row }
    $storagePackageTestPatterns = [ordered]@{
        'P5-T-C057-PUBLIC'='(?i)/var/lib/bcsp.*operational-only.*zero personal';
        'P5-T-C064-LOCAL'='(?i)RBCSP\.exe.*CWD.*data/rbcsp\.sqlite.*no path fallback';
        'P5-T-C065-LOCAL'='(?i)one physical data/rbcsp\.sqlite.*operational.*personal.*disjoint';
        'P5-T-C065-NOPUBLIC'='(?i)personal repository migration registry table definitions.*absent';
        'P5-T-C072-LOCAL'='(?i)zero DB SQLite WAL SHM seed.*real Catalog Open.*first run';
        'P5-T-C074-PUBLIC'='(?i)/var/lib/bcsp.*zero DB SQLite WAL SHM seed real Catalog Open.*personal schema';
        'P5-T-C076-NOLOCAL'='(?i)zero prebuilt DB SQLite WAL SHM seed real Catalog Open';
        'P5-T-C076-NOPUBLIC'='(?i)zero prebuilt DB SQLite WAL SHM seed real Catalog Open'
    }
    foreach ($entry in $storagePackageTestPatterns.GetEnumerator()) {
        Assert-True ($testById.ContainsKey($entry.Key)) "P5 storage/package test missing: $($entry.Key)"
        if ($testById.ContainsKey($entry.Key)) {
            Assert-True (([string]$testById[$entry.Key].build_assertion) -match $entry.Value) "P5 storage/package assertion drifted: $($entry.Key)"
        }
    }

    # The conflict ledger is a closed decision ledger, including every build
    # surface through which LOCAL_ONLY code might otherwise leak publicly.
    $conflictPath = Join-Path $p5 '05-conflict-and-leakage-ledger.tsv'
    $conflictHeaders = @('conflict_id','conflict_class','source_refs','issue','authoritative_resolution','semantic_owner','boundary','regression_test_id','p7_task_ids','status')
    Assert-Headers $conflictPath $conflictHeaders 'P5 conflict ledger'
    $conflicts = @(Import-Csv -LiteralPath $conflictPath -Delimiter "`t" -Encoding utf8)
    Assert-True ($conflicts.Count -eq 12) "P5 conflict ledger expected 12 rows, got $($conflicts.Count)"
    Assert-ColumnsNonBlank $conflicts $conflictHeaders 'P5 conflict ledger'
    Assert-True (@($conflicts.conflict_id | Sort-Object -Unique).Count -eq $conflicts.Count) 'P5 conflict_id is not unique'
    Assert-True (@($conflicts.regression_test_id | Sort-Object -Unique).Count -eq $conflicts.Count) 'P5 conflict regression_test_id is not unique'
    Assert-True (@($conflicts | Where-Object { $_.status -ne 'RESOLVED' }).Count -eq 0) 'P5 conflict ledger has unresolved rows'
    $expectedConflictClasses = @(
        'SEMANTIC_CONFLICT','CODE_FORK','SOURCE_LEAKAGE','ROUTE_LEAKAGE','API_LEAKAGE','STORAGE_LEAKAGE','I18N_LEAKAGE',
        'BUNDLE_LEAKAGE','PACKAGE_LEAKAGE','STATE_OWNERSHIP_CONFLICT','OPEN_POLICY_CONFLICT','DEPLOYMENT_AUTHORITY_CONFLICT'
    )
    Assert-SameSet @($conflicts.conflict_class) $expectedConflictClasses 'P5 conflict classes'
    $conflictCorpus = (($conflicts | ForEach-Object { "$($_.conflict_class) $($_.issue) $($_.authoritative_resolution) $($_.boundary)" }) -join "`n")
    foreach ($surface in @('SOURCE','ROUTE','API','STORAGE','I18N','BUNDLE','PACKAGE')) {
        Assert-True ($conflictCorpus -match "(?i)(?<![A-Za-z0-9])$surface(?![A-Za-z0-9])") "P5 conflict ledger does not cover $surface leakage"
    }
    $storageConflict = @($conflicts | Where-Object { $_.conflict_id -eq 'P5-L006' })
    $packageConflict = @($conflicts | Where-Object { $_.conflict_id -eq 'P5-L009' })
    Assert-True ($storageConflict.Count -eq 1) 'P5 storage leakage decision row is missing or duplicated'
    if ($storageConflict.Count -eq 1) {
        $resolution = [string]$storageConflict[0].authoritative_resolution
        Assert-True ($resolution -match [regex]::Escape($storageDecisionId)) 'P5-L006 does not cite the P6 Review storage amendment'
        Assert-True ($resolution -match '(?i)data/rbcsp\.sqlite.*disjoint operational.*personal.*CWD.*fallback.*?/var/lib/bcsp.*operational-only.*zero personal') 'P5-L006 does not close single-file logical ownership, path fallback, and public personal-schema leakage'
    }
    Assert-True ($packageConflict.Count -eq 1) 'P5 package leakage decision row is missing or duplicated'
    if ($packageConflict.Count -eq 1) {
        $resolution = [string]$packageConflict[0].authoritative_resolution
        Assert-True ($resolution -match '(?i)Both deny DB SQLite WAL SHM seed.*real Catalog Open.*first-run') 'P5-L009 does not close database, sidecar, seed, and real-data package leakage'
    }

    # Traceability is a bijective union of capability, test and conflict IDs.
    $tracePath = Join-Path $p5 '06-p5-traceability-matrix.tsv'
    $traceHeaders = @('trace_id','source_kind','source_ref','requirement_id','capability_id','classification','implementation_target','verification_test_ids','p7_task_ids','p6_handoff','gate_status')
    Assert-Headers $tracePath $traceHeaders 'P5 traceability'
    $trace = @(Import-Csv -LiteralPath $tracePath -Delimiter "`t" -Encoding utf8)
    Assert-True ($trace.Count -eq 194) "P5 traceability expected 194 rows, got $($trace.Count)"
    Assert-ColumnsNonBlank $trace $traceHeaders 'P5 traceability'
    Assert-True (@($trace.trace_id | Sort-Object -Unique).Count -eq $trace.Count) 'P5 trace_id is not unique'
    $allowedSourceKinds = @('CAPABILITY_CLASSIFICATION','TEST_CONTRACT','RESOLVED_CONFLICT')
    Assert-True (@($trace | Where-Object { $_.source_kind -notin $allowedSourceKinds }).Count -eq 0) 'P5 traceability contains an invalid source_kind'
    $capabilityTrace = @($trace | Where-Object { $_.source_kind -eq 'CAPABILITY_CLASSIFICATION' })
    $testTrace = @($trace | Where-Object { $_.source_kind -eq 'TEST_CONTRACT' })
    $conflictTrace = @($trace | Where-Object { $_.source_kind -eq 'RESOLVED_CONFLICT' })
    Assert-True ($capabilityTrace.Count -eq 76 -and $testTrace.Count -eq 106 -and $conflictTrace.Count -eq 12) 'P5 trace source-kind counts drifted'
    Assert-SameSet @($capabilityTrace.requirement_id) @($capabilities.capability_id) 'P5 capability trace requirement coverage'
    Assert-SameSet @($capabilityTrace.verification_test_ids) @($capabilities.acceptance_test_id) 'P5 capability trace verification coverage'
    Assert-SameSet @($testTrace.source_ref) @($tests | ForEach-Object { "04:$($_.test_row_id)" }) 'P5 test trace source coverage'
    Assert-SameSet @($testTrace.requirement_id) @($tests.test_id) 'P5 test trace requirement coverage'
    Assert-SameSet @($testTrace.verification_test_ids) @($tests.test_id) 'P5 test trace verification coverage'
    Assert-SameSet @($conflictTrace.source_ref) @($conflicts | ForEach-Object { "05:$($_.conflict_id)" }) 'P5 conflict trace source coverage'
    Assert-SameSet @($conflictTrace.requirement_id) @($conflicts.regression_test_id) 'P5 conflict trace requirement coverage'
    Assert-SameSet @($conflictTrace.verification_test_ids) @($conflicts.regression_test_id) 'P5 conflict trace verification coverage'
    foreach ($row in $capabilityTrace) {
        $source = @($capabilities | Where-Object { $_.capability_id -eq $row.requirement_id })
        if ($source.Count -eq 1) {
            Assert-True ($row.capability_id -eq $source[0].capability_id) "P5 capability trace capability_id drift for $($row.source_ref)"
            Assert-True ($row.classification -eq $source[0].classification) "P5 capability trace classification drift for $($row.source_ref)"
            Assert-True ($row.source_ref -match ('(?<![A-Za-z0-9])' + [regex]::Escape($source[0].p4_delta_id) + '(?![A-Za-z0-9])')) "P5 capability trace omits P4 delta source for $($row.requirement_id)"
            if ($row.requirement_id -in @('P5-C057','P5-C064','P5-C065','P5-C072','P5-C074','P5-C076')) {
                Assert-True ($row.source_ref -match [regex]::Escape($storageDecisionId)) "P5 capability trace omits storage amendment source for $($row.requirement_id)"
                Assert-True ($row.implementation_target -eq $source[0].module_boundary) "P5 capability trace storage/package target drift for $($row.requirement_id)"
            }
        }
    }
    foreach ($row in $testTrace) {
        $testRowId = ([string]$row.source_ref -replace '^04:', '')
        $source = @($tests | Where-Object { $_.test_row_id -eq $testRowId })
        if ($source.Count -eq 1) {
            Assert-True ($row.capability_id -eq $source[0].capability_id) "P5 test trace capability_id drift for $($row.source_ref)"
            Assert-True ($row.classification -eq $source[0].classification) "P5 test trace classification drift for $($row.source_ref)"
            if ($source[0].test_id -in $storagePackageTestPatterns.Keys) {
                Assert-True ($row.implementation_target -eq $source[0].build_assertion) "P5 test trace storage/package description drift for $($row.source_ref)"
            }
        }
    }
    $storageConflictTrace = @($conflictTrace | Where-Object { $_.source_ref -eq '05:P5-L006' })
    $packageConflictTrace = @($conflictTrace | Where-Object { $_.source_ref -eq '05:P5-L009' })
    Assert-True ($storageConflictTrace.Count -eq 1 -and $storageConflictTrace[0].implementation_target -match 'LOCAL_SINGLE_DB_LOGICAL_DOMAINS_PUBLIC_OPERATIONAL_ONLY') 'P5 storage conflict trace description drifted'
    Assert-True ($packageConflictTrace.Count -eq 1 -and $packageConflictTrace[0].implementation_target -match 'ZERO_DATABASE_AND_REAL_DATA_PAYLOAD') 'P5 package conflict trace description drifted'

    $reviewGate = Read-Text $p5 '07-p5-validation-gate.md'
    $record = Read-Json $p5 '07a-p5-validation-gate.json'
    Assert-PropertyEquals $record 'phase' 'P5' 'P5 validation record'
    Assert-PropertyEquals $record 'p4InputGate' 'P4_PASS' 'P5 validation record'
    $reviewDecision = Get-Property $record 'reviewDecision'
    Assert-PropertyEquals $reviewDecision 'id' $storageDecisionId 'P5 validation record reviewDecision'
    Assert-PropertyEquals $reviewDecision 'date' '2026-07-13' 'P5 validation record reviewDecision'
    Assert-True (-not [bool](Get-Property $reviewDecision 'p7Authorized')) 'P5 storage amendment review decision authorizes P7'
    $counts = Get-Property $record 'matrixCounts'
    foreach ($entry in ([ordered]@{
        capabilityRows=76; sharedRows=46; localOnlyRows=9; publicOnlyRows=19; excludedRows=2;
        testRows=106; conflictRows=12; unresolvedConflicts=0; traceRows=194
    }).GetEnumerator()) {
        Assert-PropertyEquals $counts $entry.Key $entry.Value 'P5 validation record matrixCounts'
    }
    $architecture = Get-Property $record 'architecture'
    Assert-PropertyEquals $architecture 'sharedBusinessLogicImplementationCount' 1 'P5 validation record architecture'
    Assert-PropertyEquals $architecture 'longLivedForkCount' 0 'P5 validation record architecture'
    $storageBoundary = Get-Property $architecture 'storageBoundary'
    Assert-PropertyEquals $storageBoundary 'localPhysicalDatabaseCount' 1 'P5 validation record storageBoundary'
    Assert-PropertyEquals $storageBoundary 'localDatabaseRelativePath' 'data/rbcsp.sqlite' 'P5 validation record storageBoundary'
    Assert-PropertyEquals $storageBoundary 'localPathAnchor' 'RESOLVED_EXECUTABLE_ROOT' 'P5 validation record storageBoundary'
    Assert-True ([bool](Get-Property $storageBoundary 'localCwdIndependent')) 'P5 validation record storageBoundary allows CWD-dependent local storage'
    Assert-True (-not [bool](Get-Property $storageBoundary 'localFallbackAllowed')) 'P5 validation record storageBoundary allows local path fallback'
    Assert-SameSet @((Get-Property $storageBoundary 'localLogicalDomains')) @('OPERATIONAL','LOCAL_ONLY_PERSONAL') 'P5 validation record local storage domains'
    Assert-PropertyEquals $storageBoundary 'publicStateRoot' '/var/lib/bcsp' 'P5 validation record storageBoundary'
    Assert-True ([bool](Get-Property $storageBoundary 'publicOperationalOnly')) 'P5 validation record public storage is not operational-only'
    Assert-True (-not [bool](Get-Property $storageBoundary 'publicPersonalMigrationsLinked')) 'P5 validation record links personal migrations into public'
    Assert-True (-not [bool](Get-Property $storageBoundary 'publicPersonalTablesLinked')) 'P5 validation record links personal tables into public'
    $packageBoundary = Get-Property $architecture 'packageBoundary'
    foreach ($property in @('archiveDatabaseFiles','archiveWalShmFiles','archiveSeedFiles','archiveRealCatalogOpenPayloads')) {
        Assert-PropertyEquals $packageBoundary $property 0 'P5 validation record packageBoundary'
    }
    Assert-True ([bool](Get-Property $packageBoundary 'runtimeStateCreatedAfterFirstStart')) 'P5 validation record permits precreated runtime state in release archives'
    $sideEffects = Get-Property $record 'sideEffects'
    foreach ($property in @('rutgersRequests','networkEvidenceRequests','productSourceMutations','dependencyMutations','databaseMutations','packageBuilds','releasePublications','productionMutations')) {
        Assert-PropertyEquals $sideEffects $property 0 'P5 validation record sideEffects'
    }
    $eligibility = Get-Property $record 'eligibility'
    Assert-True (-not [bool](Get-Property $eligibility 'p7')) 'P5 validation record authorizes P7'

    $commonGateMarkers = [ordered]@{
        phase='P5'; p4_input_gate='P4_PASS'; review_storage_amendment=$storageDecisionId;
        capability_rows='76'; shared_rows='46'; local_only_rows='9';
        public_only_rows='19'; excluded_rows='2'; test_rows='106'; conflict_rows='12'; unresolved_conflicts='0';
        trace_rows='194'; shared_business_logic_implementation_count='1'; long_lived_fork_count='0';
        local_physical_database_count='1'; local_database_relative_path='data/rbcsp.sqlite';
        local_storage_path_anchor='RESOLVED_EXECUTABLE_ROOT'; local_storage_cwd_independent='TRUE';
        local_storage_fallback_allowed='FALSE'; local_operational_personal_logical_domains='2';
        public_state_root='/var/lib/bcsp'; public_personal_migrations_linked='FALSE'; public_personal_tables_linked='FALSE';
        release_archive_database_files='0'; release_archive_seed_or_real_data_files='0';
        rutgers_requests='0'; product_source_mutations='0'; package_builds='0'; production_mutations='0'; p7_authorized='FALSE'
    }
    foreach ($entry in $commonGateMarkers.GetEnumerator()) { Assert-Marker $reviewGate $entry.Key $entry.Value 'P5 07 gate' }

    if ($ValidationMode -eq 'Candidate') {
        Assert-PropertyEquals $record 'recordStatus' 'P5_PASS_CANDIDATE' 'P5 candidate validation record'
        Assert-PropertyEquals $record 'p5Gate' 'P5_PASS_CANDIDATE' 'P5 candidate validation record'
        Assert-True (-not [bool](Get-Property $eligibility 'p6')) 'P5 candidate record prematurely authorizes P6'
        Assert-PropertyEquals $eligibility 'p6Reason' 'PENDING_FINAL_P5_GATE' 'P5 candidate eligibility'
        Assert-Marker $reviewGate 'record_status' 'P5_PASS_CANDIDATE' 'P5 candidate gate'
        Assert-Marker $reviewGate 'p5_gate' 'P5_PASS_CANDIDATE' 'P5 candidate gate'
        Assert-Marker $reviewGate 'p6_eligible' 'FALSE' 'P5 candidate gate'
        Assert-Marker $reviewGate 'p6_reason' 'PENDING_FINAL_P5_GATE' 'P5 candidate gate'
    }
    else {
        Assert-PropertyEquals $record 'recordStatus' 'P5_PASS' 'P5 final validation record'
        Assert-PropertyEquals $record 'p5Gate' 'P5_PASS' 'P5 final validation record'
        Assert-True ([bool](Get-Property $eligibility 'p6')) 'P5 final validation record does not authorize P6'
        Assert-Marker $reviewGate 'record_status' 'P5_PASS' 'P5 final gate'
        Assert-Marker $reviewGate 'p5_gate' 'P5_PASS' 'P5 final gate'
        Assert-Marker $reviewGate 'p6_eligible' 'TRUE' 'P5 final gate'
    }

    if ($failures.Count -gt 0) {
        foreach ($failure in $failures) { Write-Output "VALIDATION_FAILURE=$failure" }
        Write-Output 'p5_gate=FAIL'
        Write-Output 'p6_eligibility=FALSE'
        Write-Output "integrity_failures=$($failures.Count)"
        exit 1
    }

    if ($ValidationMode -eq 'Candidate') {
        Write-Output 'p5_gate=P5_PASS_CANDIDATE'
        Write-Output 'p6_eligibility=FALSE'
    }
    else {
        Write-Output 'p5_gate=P5_PASS'
        Write-Output 'p6_eligibility=TRUE'
    }
    Write-Output 'integrity_failures=0'
}
finally {
    Pop-Location
}
