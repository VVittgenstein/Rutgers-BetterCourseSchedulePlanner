param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path,
    [ValidateSet('Candidate','Final')]
    [string]$ValidationMode = 'Final'
)

$ErrorActionPreference = 'Stop'
$p4 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$p3 = (Resolve-Path (Join-Path $p4 '..\p3')).Path
$failures = [System.Collections.Generic.List[string]]::new()

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

function Assert-Subset([object[]]$Actual, [object[]]$Required, [string]$Label) {
    $actualSet = @($Actual | ForEach-Object { [string]$_ } | Where-Object { $_ -ne '' } | Sort-Object -Unique)
    $missing = @($Required | ForEach-Object { [string]$_ } | Where-Object { $_ -ne '' -and $_ -notin $actualSet } | Sort-Object -Unique)
    Assert-True ($missing.Count -eq 0) "$Label missing: $($missing -join ',')"
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Assert-Hash([string]$RelativePath, [string]$ExpectedHash) {
    $path = Join-Path $p3 $RelativePath
    Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "missing pinned P3 input: $RelativePath"
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        Assert-True ((Get-Sha256 $path) -eq $ExpectedHash) "pinned P3 hash drifted: $RelativePath"
    }
}

function Read-Text([string]$BasePath, [string]$RelativePath) {
    return Get-Content -LiteralPath (Join-Path $BasePath $RelativePath) -Raw -Encoding utf8
}

function Read-Json([string]$BasePath, [string]$RelativePath) {
    return Read-Text $BasePath $RelativePath | ConvertFrom-Json
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

function Split-IdentifierList([object]$Value) {
    if ($null -eq $Value) { return @() }
    return @(([string]$Value -split '[\s,;|]+') | Where-Object { $_ -ne '' })
}

function Get-Property([object]$Object, [string]$Name) {
    if ($null -eq $Object) { return $null }
    $property = @($Object.PSObject.Properties | Where-Object { $_.Name -eq $Name })
    if ($property.Count -eq 1) { return $property[0].Value }
    return $null
}

Push-Location $RepositoryRoot
try {
    # P4 is allowed to inherit only from these byte-pinned, already-PASS P3
    # inputs. Any later edit must first create a new reviewed baseline.
    $p3Pins = [ordered]@{
        '07-local-oneclick-implementation-plan.md' = '26B6805284E50639F91FD6523365FD8C0A9607353A0EF6B99D20B6E8910AA634'
        '08-p3-traceability-matrix.tsv' = '234D2C8091B126CC594D6EA406AB9214339FD62FDC0348CDF60D360F434971EE'
        '09-p3-validation-and-freeze-gate.md' = '7CCB3CB067781961A17E7910D150325CC34B9390CEFEAE18E4F25A4E48CD9851'
        '22a-open-round2-completion.md' = '20BC18B5B2ABAC82F97C559C02F0D6B7F5EE08792F710637E3790B62D85C9E6D'
        '22b-open-round2-completion.json' = '7014E38B6A142CD653CA387AD062D82176A7F372767ADADE2E7A47E123E88F94'
        '23a-shared-open-final-contract.md' = '73031DD718A346D659214D6BE4C571D505836098DE42879E8861349CBD73887E'
        '23b-shared-open-final-contract.json' = '9BFE6712E959C39B6719E7A47F7F96E33AAC5BD20A8CFEF51320C0E2A0C77804'
        '11-open-request-ledger.tsv' = 'DC7A44570C727AA3561A2468A361AB27191807E87A781E41EF81EFF730B7977E'
    }
    foreach ($entry in $p3Pins.GetEnumerator()) { Assert-Hash $entry.Key $entry.Value }

    $p3Gate = Read-Text $p3 '09-p3-validation-and-freeze-gate.md'
    Assert-Marker $p3Gate 'p3_gate' 'P3_PASS' 'P3 09 gate'
    Assert-Marker $p3Gate 'p4_eligibility' 'TRUE' 'P3 09 gate'
    Assert-Marker $p3Gate 'p7_authorized' 'FALSE' 'P3 09 gate'
    Assert-Marker $p3Gate 'further_p3_network_requests_authorized' '0' 'P3 09 gate'

    $p3Completion = Read-Json $p3 '22b-open-round2-completion.json'
    Assert-True ($p3Completion.status -eq 'FROZEN_ROUND_2_COMPLETE') 'P3 Open completion is not frozen'
    Assert-True ([int]$p3Completion.attempts.completed -eq 42 -and [int]$p3Completion.attempts.successful -eq 42) 'P3 Open completion totals drifted'
    Assert-True ([int]$p3Completion.furtherP3EvidenceRequestsAuthorized -eq 0) 'P3 completion authorizes further Rutgers evidence requests'
    Assert-True ($p3Completion.hashes.finalLedgerSha256 -eq $p3Pins['11-open-request-ledger.tsv']) 'P3 completion-to-ledger hash chain drifted'

    $p3OpenContract = Read-Json $p3 '23b-shared-open-final-contract.json'
    Assert-True ($p3OpenContract.status -eq 'FROZEN_P3_SHARED_OPEN_CONTRACT') 'P3 shared Open contract is not frozen'
    Assert-True ([int]$p3OpenContract.evidence.additionalRequestsAuthorized -eq 0) 'P3 shared Open contract authorizes additional requests'

    $requiredP4 = @(
        '00-p4-charter-and-preflight.md',
        '01-public-baseline-delta-matrix.tsv',
        '02-public-runtime-session-capability-contract.md',
        '03-public-refresh-capacity-degradation-plan.md',
        '04-linux-package-and-operations-plan.md',
        '05-public-capability-deny-audit.tsv',
        '06-p4-traceability-matrix.tsv',
        '07-p4-validation-gate.md',
        '07a-p4-validation-gate.json'
    )
    foreach ($name in $requiredP4) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p4 $name) -PathType Leaf) "missing P4 artifact: $name"
    }

    # P4 is a design-only phase. Evidence payloads, request ledgers and network
    # captures are categorically out of scope, not merely expected to be empty.
    $networkArtifactPattern = '(?i)(request[-_]?ledger|rutgers[-_]?(request|response)|raw[-_]?(body|response)|network[-_]?capture|http[-_]?capture|evidence[-_]?payload|\.har$|\.body$)'
    $p4NetworkArtifacts = @(Get-ChildItem -LiteralPath $p4 -Recurse -File | Where-Object { $_.FullName -match $networkArtifactPattern })
    Assert-True ($p4NetworkArtifacts.Count -eq 0) "P4 Rutgers/network request artifacts must be zero: $($p4NetworkArtifacts.Name -join ',')"

    $charter = Read-Text $p4 '00-p4-charter-and-preflight.md'
    $runtime = Read-Text $p4 '02-public-runtime-session-capability-contract.md'
    $refresh = Read-Text $p4 '03-public-refresh-capacity-degradation-plan.md'
    $operations = Read-Text $p4 '04-linux-package-and-operations-plan.md'

    $charterMarkers = [ordered]@{
        phase='P4'; status='P4_CORE_DESIGN_FROZEN'; p3_input_gate='P3_PASS';
        p3_key_inputs_pinned='8'; p4_scope='PUBLIC_PACKAGE_AND_DEPLOYMENT_PLAN';
        rutgers_requests_authorized='0'; p4_rutgers_request_artifacts='0';
        product_source_changes_authorized='0'; production_changes_authorized='0';
        production_mutations='0'; deployment_execution_authorized='FALSE';
        deployment_is_third_package='FALSE'
    }
    foreach ($entry in $charterMarkers.GetEnumerator()) { Assert-Marker $charter $entry.Key $entry.Value 'P4 00 charter' }

    $runtimeMarkers = [ordered]@{
        status='P4_PUBLIC_RUNTIME_CONTRACT_FROZEN';
        public_session='NEW_ON_EVERY_TOP_LEVEL_DOCUMENT_LOAD';
        public_personal_persistence='NONE'; user_state='EPHEMERAL'; saved_views='ABSENT';
        saved_views_zero_surface='TRUE'; language_init='SYSTEM_OR_BROWSER_EACH_LOAD';
        browser_direct_rutgers='FALSE'; new_document_upstream_amplification='FALSE';
        upstream_architecture='CENTRAL_POLLER_WEBSOCKET';
        public_operational_state='SERVICE_WIDE_NON_PERSONAL'
    }
    foreach ($entry in $runtimeMarkers.GetEnumerator()) { Assert-Marker $runtime $entry.Key $entry.Value 'P4 02 runtime contract' }

    $refreshMarkers = [ordered]@{
        status='P4_PUBLIC_REFRESH_CONTRACT_FROZEN'; catalog_interval_sec='600';
        open_general_interval_sec='30'; open_active_target_sec='10'; open_batch_key='term_campus';
        per_target_single_flight='TRUE'; shared_catalog_open_origin_limiter='TRUE';
        user_or_watch_count_amplifies_requests='FALSE'; real_origin_concurrency='1';
        scheduler='EDF_NO_STARVATION'; counter_scope='SERVICE_WIDE';
        counter_timezone='AMERICA_NEW_YORK'; valid_observation_to_server_fanout_target_sec='1';
        hard_fanout_sla='FALSE'; hard_real_change_to_notification_30s='FALSE'
    }
    foreach ($entry in $refreshMarkers.GetEnumerator()) { Assert-Marker $refresh $entry.Key $entry.Value 'P4 03 refresh contract' }

    $operationsMarkers = [ordered]@{
        status='P4_PUBLIC_PACKAGE_OPERATIONS_PLAN_FROZEN'; target_os='UBUNTU_24_04';
        storage_amendment='P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001'; storage_amendment_date='2026-07-13';
        service_manager='SYSTEMD'; edge_proxy='CADDY'; https='REQUIRED';
        backup_plan='REQUIRED'; restore_plan='REQUIRED';
        linux_state_root='/var/lib/bcsp'; public_operational_database='/var/lib/bcsp/rbcsp.sqlite';
        first_start_database='SCHEMA_ONLY_OPERATIONAL'; both_package_first_start_databases='SCHEMA_ONLY_OPERATIONAL';
        archive_database_files='0';
        archive_wal_shm_files='0'; archive_seed_fixture_files='0'; archive_real_catalog_open_data='0';
        local_personal_migrations_in_public='FALSE'; production_deployment_authorized='FALSE';
        p7_authorized='FALSE'; deployment_is_third_package='FALSE'
    }
    foreach ($entry in $operationsMarkers.GetEnumerator()) { Assert-Marker $operations $entry.Key $entry.Value 'P4 04 operations plan' }

    # Shared-baseline -> public delta matrix.
    $deltaPath = Join-Path $p4 '01-public-baseline-delta-matrix.tsv'
    Assert-Headers $deltaPath @('row_id','input_ref','capability','disposition','public_contract','reason','consumers','validation_ids','p7_task_ids') 'P4 delta matrix'
    $delta = @(Import-Csv -LiteralPath $deltaPath -Delimiter "`t" -Encoding utf8)
    Assert-True ($delta.Count -eq 76) "P4 delta matrix expected 76 rows, got $($delta.Count)"
    Assert-True (@($delta | Where-Object { [string]::IsNullOrWhiteSpace($_.row_id) }).Count -eq 0) 'P4 delta matrix has blank row_id'
    Assert-True ((@($delta.row_id | Sort-Object -Unique)).Count -eq $delta.Count) 'P4 delta matrix row_id is not unique'
    $allowedDispositions = @('INHERIT_SHARED','PUBLIC_MODIFY','PUBLIC_EXCLUDE')
    Assert-True (@($delta | Where-Object { $_.disposition -notin $allowedDispositions }).Count -eq 0) 'P4 delta matrix contains an invalid disposition'
    Assert-SameSet @($delta.disposition) $allowedDispositions 'P4 delta dispositions represented'
    Assert-True (@($delta | Where-Object { $_.disposition -eq 'INHERIT_SHARED' }).Count -eq 46) 'P4 INHERIT_SHARED row count drifted'
    Assert-True (@($delta | Where-Object { $_.disposition -eq 'PUBLIC_MODIFY' }).Count -eq 19) 'P4 PUBLIC_MODIFY row count drifted'
    Assert-True (@($delta | Where-Object { $_.disposition -eq 'PUBLIC_EXCLUDE' }).Count -eq 11) 'P4 PUBLIC_EXCLUDE row count drifted'
    Assert-True (@($delta | Where-Object { [string]::IsNullOrWhiteSpace($_.validation_ids) }).Count -eq 0) 'P4 delta matrix has blank validation_ids'

    $storageDeltaPatterns = [ordered]@{
        'P4-B057' = @('/var/lib/bcsp/rbcsp\.sqlite','(?i)schema-only','(?i)public migration graph contains no LOCAL_ONLY personal tables or migrations')
        'P4-B064' = @('data/rbcsp\.sqlite','/var/lib/bcsp')
        'P4-B065' = @('(?i)LOCAL_ONLY personal tables and migrations','data/rbcsp\.sqlite','(?i)public build-reachable migration')
        'P4-B072' = @('data/rbcsp\.sqlite','(?i)distinct storage roots')
        'P4-B074' = @('/var/lib/bcsp/rbcsp\.sqlite','(?i)no database WAL SHM seed fixture or real Catalog/Open data','(?i)schema-only')
        'P4-B076' = @('(?i)every database or DB sample WAL SHM seed fixture','(?i)real or cached Catalog/Open','(?i)data-empty')
    }
    foreach ($entry in $storageDeltaPatterns.GetEnumerator()) {
        $rows = @($delta | Where-Object { $_.row_id -eq $entry.Key })
        Assert-True ($rows.Count -eq 1) "P4 storage delta row missing or duplicated: $($entry.Key)"
        if ($rows.Count -eq 1) {
            $row = $rows[0]
            Assert-True ($row.input_ref -match '(?i)(?:^|;)P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001(?:;|$)') "P4 storage delta row does not cite amendment: $($entry.Key)"
            $rowText = "$($row.capability) $($row.public_contract) $($row.reason) $($row.consumers)"
            foreach ($pattern in $entry.Value) {
                Assert-True ([regex]::IsMatch($rowText, $pattern)) "P4 storage delta contract drifted: $($entry.Key) missing $pattern"
            }
        }
    }

    $storageDocuments = $operations + "`n" + (Read-Text $p4 '07-p4-validation-gate.md')
    Assert-True (-not [regex]::IsMatch($storageDocuments, '(?i)Known Folder|user\.sqlite|catalog\.sqlite')) 'P4 revised storage contracts retain a superseded multi-file/Known Folder model'
    Assert-True ([regex]::IsMatch($storageDocuments, '(?i)database.*WAL.*SHM.*seed.*fixture.*Catalog.*Open')) 'P4 archive data-empty deny contract is incomplete'

    $deltaValidationIds = @($delta | ForEach-Object { Split-IdentifierList $_.validation_ids })
    Assert-True ($deltaValidationIds.Count -eq 78 -and @($deltaValidationIds | Sort-Object -Unique).Count -eq 78) 'P4 delta validation IDs must be exactly 78 unique IDs'
    $requiredFilterIds = @(
        'FLT-C01','FLT-C02','FLT-C03','FLT-C04','FLT-C05','FLT-C06','FLT-C07','FLT-C08','FLT-C09','FLT-C10',
        'FLT-S01','FLT-S02','FLT-S03','FLT-S04a','FLT-S04b','FLT-S05','FLT-S06','FLT-S07','FLT-S08','FLT-S09','FLT-S10','FLT-S11'
    )
    $deltaFilterText = (($delta | ForEach-Object { "$($_.capability) $($_.validation_ids)" }) -join "`n")
    foreach ($filterId in $requiredFilterIds) {
        $filterPattern = '(?<![A-Za-z0-9])' + [regex]::Escape($filterId) + '(?![A-Za-z0-9])'
        Assert-True ([regex]::IsMatch($deltaFilterText, $filterPattern)) "P4 delta matrix missing filter $filterId"
    }

    $deltaProductText = (($delta | ForEach-Object { "$($_.input_ref) $($_.capability) $($_.public_contract) $($_.validation_ids)" }) -join "`n")
    $keyCapabilityPatterns = [ordered]@{
        OPEN='(?i)OPEN'; SUBSCRIPTION='(?i)(SUBSCRIPTION|WATCH)';
        AUDIO_NOTIFICATION='(?i)(AUDIO|NOTIFICATION)'; I18N='(?i)(I18N|LANGUAGE)';
        RESET='(?i)RESET'; SAVED_VIEWS='(?i)SAVED[_ -]?VIEWS?';
        PERSISTENCE='(?i)PERSIST'; COURSE='(?i)COURSE'; SECTION='(?i)SECTION'
    }
    foreach ($entry in $keyCapabilityPatterns.GetEnumerator()) {
        Assert-True ([regex]::IsMatch($deltaProductText, $entry.Value)) "P4 delta matrix does not cover key capability $($entry.Key)"
    }

    # Every excluded capability must be absent from every public surface.
    $denyPath = Join-Path $p4 '05-public-capability-deny-audit.tsv'
    Assert-Headers $denyPath @('deny_id','capability','surface','forbidden_public_surface','allowed_exception','status','validation_id','p7_task_id') 'P4 deny audit'
    $deny = @(Import-Csv -LiteralPath $denyPath -Delimiter "`t" -Encoding utf8)
    $expectedSurfaces = @('SOURCE','DOM','ROUTE','API','STORAGE','I18N','BUNDLE','PACKAGE')
    $denyCapabilities = @($deny.capability | Sort-Object -Unique)
    Assert-True ($denyCapabilities.Count -eq 18) "P4 deny audit expected 18 capabilities, got $($denyCapabilities.Count)"
    Assert-True ($deny.Count -eq 144) "P4 deny audit expected 144 capability-by-surface rows, got $($deny.Count)"
    Assert-True ((@($deny.deny_id | Sort-Object -Unique)).Count -eq $deny.Count) 'P4 deny_id is not unique'
    $denyPairs = @($deny | ForEach-Object { "$($_.capability)|$($_.surface)" })
    Assert-True ((@($denyPairs | Sort-Object -Unique)).Count -eq $deny.Count) 'P4 deny capability/surface pair is not unique'
    Assert-True (@($deny | Where-Object { $_.status -ne 'PUBLIC_ZERO_SURFACE' }).Count -eq 0) 'P4 deny audit contains a non-zero public surface'
    Assert-True (@($deny | Where-Object { [string]::IsNullOrWhiteSpace($_.validation_id) }).Count -eq 0) 'P4 deny audit has blank validation_id'
    Assert-SameSet @($deny.surface) $expectedSurfaces 'P4 deny surface universe'
    foreach ($capability in $denyCapabilities) {
        Assert-SameSet @($deny | Where-Object { $_.capability -eq $capability } | ForEach-Object { $_.surface }) $expectedSurfaces "P4 deny surfaces for $capability"
    }
    $savedViewCapabilities = @($denyCapabilities | Where-Object { $_ -match '(?i)SAVED[_ -]?VIEWS?' })
    Assert-True ($savedViewCapabilities.Count -ge 1) 'P4 deny audit does not include Saved views'

    # Traceability must cover every delta/deny validation target and contain no
    # dangling source or consumer cell.
    $tracePath = Join-Path $p4 '06-p4-traceability-matrix.tsv'
    Assert-Headers $tracePath @('trace_id','source_kind','source_ref','requirement_or_test','disposition','p4_consumer','p7_task_id','verification_gate') 'P4 traceability'
    $trace = @(Import-Csv -LiteralPath $tracePath -Delimiter "`t" -Encoding utf8)
    Assert-True ($trace.Count -eq 258) "P4 traceability expected 258 rows, got $($trace.Count)"
    Assert-True ((@($trace.trace_id | Sort-Object -Unique)).Count -eq $trace.Count) 'P4 trace_id is not unique'
    foreach ($column in @('trace_id','source_kind','source_ref','requirement_or_test','disposition','p4_consumer','p7_task_id','verification_gate')) {
        Assert-True (@($trace | Where-Object { [string]::IsNullOrWhiteSpace($_.$column) }).Count -eq 0) "P4 traceability has blank $column"
    }
    $coreContractMatches = [regex]::Matches(($runtime + "`n" + $refresh + "`n" + $operations), '(?<![A-Za-z0-9])P4-(?:SESSION|REFRESH|OPS)-\d{3}(?![A-Za-z0-9])')
    $coreContractIds = @($coreContractMatches | ForEach-Object { $_.Value } | Sort-Object -Unique)
    Assert-True ($coreContractIds.Count -eq 30) "P4 core contracts expected 30 acceptance IDs, got $($coreContractIds.Count)"
    foreach ($prefix in @('P4-SESSION-','P4-REFRESH-','P4-OPS-')) {
        Assert-True (@($coreContractIds | Where-Object { $_.StartsWith($prefix) }).Count -eq 10) "P4 core contract $prefix acceptance count drifted"
    }

    $baselineTrace = @($trace | Where-Object { $_.source_kind -eq 'BASELINE_DELTA_TEST' })
    $zeroSurfaceTrace = @($trace | Where-Object { $_.source_kind -eq 'PUBLIC_ZERO_SURFACE_TEST' })
    $coreContractTrace = @($trace | Where-Object { $_.source_kind -eq 'CORE_CONTRACT_TEST' })
    $frozenInputTrace = @($trace | Where-Object { $_.source_kind -eq 'FROZEN_INPUT' })
    Assert-True ($baselineTrace.Count -eq 78 -and $zeroSurfaceTrace.Count -eq 144 -and $coreContractTrace.Count -eq 30 -and $frozenInputTrace.Count -eq 6) 'P4 trace source-kind counts drifted'
    Assert-True (@($trace | Where-Object { $_.source_kind -notin @('FROZEN_INPUT','BASELINE_DELTA_TEST','PUBLIC_ZERO_SURFACE_TEST','CORE_CONTRACT_TEST') }).Count -eq 0) 'P4 trace contains an unknown source_kind'
    Assert-SameSet @($baselineTrace.source_ref) @($delta.row_id) 'P4 baseline trace source rows'
    Assert-SameSet @($baselineTrace.requirement_or_test) $deltaValidationIds 'P4 baseline trace validation IDs'
    Assert-SameSet @($zeroSurfaceTrace.source_ref) @($deny.deny_id) 'P4 zero-surface trace source rows'
    Assert-SameSet @($zeroSurfaceTrace.requirement_or_test) @($deny.validation_id) 'P4 zero-surface trace validation IDs'
    Assert-SameSet @($coreContractTrace.requirement_or_test) $coreContractIds 'P4 core-contract trace acceptance IDs'
    foreach ($row in $frozenInputTrace) {
        $sourcePath = ([string]$row.source_ref -split '#', 2)[0]
        Assert-True (Test-Path -LiteralPath (Join-Path $RepositoryRoot $sourcePath) -PathType Leaf) "P4 frozen trace source does not exist: $($row.source_ref)"
    }
    foreach ($row in $coreContractTrace) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p4 $row.source_ref) -PathType Leaf) "P4 core-contract trace source does not exist: $($row.source_ref)"
    }

    $requiredTraceRequirements = @($deltaValidationIds + @($deny | ForEach-Object { Split-IdentifierList $_.validation_id }) + $coreContractIds)
    $actualTraceRequirements = @($trace | ForEach-Object { Split-IdentifierList $_.requirement_or_test })
    Assert-Subset $actualTraceRequirements $requiredTraceRequirements 'P4 trace validation coverage'

    $reviewGate = Read-Text $p4 '07-p4-validation-gate.md'
    $record = Read-Json $p4 '07a-p4-validation-gate.json'
    Assert-Marker $reviewGate 'storage_amendment' 'P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001' 'P4 07 review gate'
    Assert-Marker $reviewGate 'storage_amendment_date' '2026-07-13' 'P4 07 review gate'
    Assert-Marker $reviewGate 'public_operational_database' '/var/lib/bcsp/rbcsp.sqlite' 'P4 07 review gate'
    Assert-Marker $reviewGate 'both_package_first_start_databases' 'SCHEMA_ONLY_OPERATIONAL' 'P4 07 review gate'
    Assert-Marker $reviewGate 'final_archive_database_files' '0' 'P4 07 review gate'
    Assert-Marker $reviewGate 'final_archive_wal_shm_files' '0' 'P4 07 review gate'
    Assert-Marker $reviewGate 'final_archive_seed_fixture_files' '0' 'P4 07 review gate'
    Assert-Marker $reviewGate 'final_archive_real_catalog_open_data' '0' 'P4 07 review gate'
    Assert-Marker $reviewGate 'public_local_personal_migrations' '0' 'P4 07 review gate'

    $storageAmendment = Get-Property $record 'storageAmendment'
    Assert-True ((Get-Property $storageAmendment 'id') -eq 'P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001') 'P4 validation record storage amendment ID drifted'
    Assert-True ((Get-Property $storageAmendment 'date') -eq '2026-07-13') 'P4 validation record storage amendment date drifted'
    Assert-True (-not [bool](Get-Property $storageAmendment 'p7Authorized')) 'P4 storage amendment authorizes P7'
    Assert-True ([bool](Get-Property $storageAmendment 'windowsPackageRelativeDatabaseExcludedFromPublic')) 'P4 storage amendment does not exclude the Windows package-relative path from public'
    Assert-True ((Get-Property $storageAmendment 'windowsDatabasePath') -eq 'data/rbcsp.sqlite') 'P4 validation record Windows relative database path drifted'
    Assert-True ((Get-Property $storageAmendment 'linuxStateRoot') -eq '/var/lib/bcsp') 'P4 validation record Linux state root drifted'
    Assert-True ((Get-Property $storageAmendment 'publicOperationalDatabase') -eq '/var/lib/bcsp/rbcsp.sqlite') 'P4 validation record public operational database drifted'
    Assert-True ([bool](Get-Property $storageAmendment 'bothPackagesFirstStartCreateSchemaOnlyOperationalDatabase')) 'P4 validation record does not enforce schema-only first start for both packages'
    Assert-True ([bool](Get-Property $storageAmendment 'publicDatabaseCreatedOnFirstStart')) 'P4 validation record does not require first-start database creation'
    Assert-True ((Get-Property $storageAmendment 'firstStartDatabaseState') -eq 'SCHEMA_ONLY_OPERATIONAL_BEFORE_UPSTREAM_INGEST') 'P4 validation record first-start database state drifted'
    Assert-True (-not [bool](Get-Property $storageAmendment 'publicMigrationGraphContainsLocalOnlyPersonalTablesOrMigrations')) 'P4 validation record permits LOCAL_ONLY personal public migrations'
    foreach ($propertyName in @('finalArchiveDatabaseFiles','finalArchiveWalShmFiles','finalArchiveSeedFixtureFiles','finalArchiveRealCatalogOpenData')) {
        Assert-True ([int](Get-Property $storageAmendment $propertyName) -eq 0) "P4 validation record final archive data count drifted: $propertyName"
    }
    Assert-True (@($record.frozenUserDecisions) -contains 'P6_REVIEW_LOCAL_STORAGE_AMENDMENT_001') 'P4 frozen decisions omit the storage amendment'
    $recordInputs = Get-Property $record 'frozenP3Inputs'
    Assert-True ([int](Get-Property $recordInputs 'count') -eq 8) 'P4 validation record frozen input count drifted'
    Assert-True ((Get-Property $recordInputs 'localPlanStatus') -eq 'P3_LOCAL_ONECLICK_PLAN_FROZEN') 'P4 validation record local plan status drifted'
    Assert-True ((Get-Property $recordInputs 'sharedOpenStatus') -eq 'FROZEN_P3_SHARED_OPEN_CONTRACT') 'P4 validation record shared Open status drifted'
    Assert-True ([int](Get-Property $recordInputs 'additionalP3RutgersRequestsAuthorized') -eq 0) 'P4 validation record expands P3 Rutgers requests'

    $recordCounts = Get-Property $record 'matrixCounts'
    $expectedRecordCounts = [ordered]@{
        baselineDeltaRows=76; inheritSharedRows=46; publicModifyRows=19; publicExcludeRows=11;
        filterContractRows=22; denyCapabilities=18; denySurfacesPerCapability=8;
        publicZeroSurfaceRows=144; traceRows=258; traceFrozenInputs=6;
        traceBaselineDeltaTests=78; tracePublicZeroSurfaceTests=144; traceCoreContractTests=30
    }
    foreach ($entry in $expectedRecordCounts.GetEnumerator()) {
        Assert-True ([int](Get-Property $recordCounts $entry.Key) -eq [int]$entry.Value) "P4 validation record count drifted: $($entry.Key)"
    }

    $recordContract = Get-Property $record 'publicContract'
    Assert-True ((Get-Property $recordContract 'topLevelDocumentSession') -eq 'NEW_EPHEMERAL_SESSION') 'P4 record public session drifted'
    Assert-True ([bool](Get-Property $recordContract 'systemLanguageEachLoad')) 'P4 record system-language initialization drifted'
    Assert-True ((Get-Property $recordContract 'savedViews') -eq 'PUBLIC_ZERO_SURFACE') 'P4 record Saved views surface drifted'
    Assert-True (-not [bool](Get-Property $recordContract 'persistentPersonalState')) 'P4 record permits persistent personal state'
    Assert-True ([int](Get-Property $recordContract 'catalogRefreshSeconds') -eq 600) 'P4 record Catalog interval drifted'
    Assert-True ([int](Get-Property $recordContract 'generalOpenRefreshSeconds') -eq 30 -and [int](Get-Property $recordContract 'activeWatchTargetSeconds') -eq 10) 'P4 record Open clocks drifted'
    Assert-True (-not [bool](Get-Property $recordContract 'publicUserRefreshConfigurable')) 'P4 record permits user-configurable public refresh'
    Assert-True (-not [bool](Get-Property $recordContract 'browserEventsAmplifyRutgersRequests')) 'P4 record permits browser request amplification'
    Assert-True ([bool](Get-Property $recordContract 'serviceRutgersDayCountersPersistAcrossRestart')) 'P4 service-wide counter persistence drifted'
    Assert-True (-not [bool](Get-Property $recordContract 'activeWatchPersistsAcrossDocumentOrRestart')) 'P4 record persists active watch across public session boundary'
    Assert-True (-not [bool](Get-Property $recordContract 'openContractForked')) 'P4 record forks the shared Open contract'

    Assert-True ([int](Get-Property $record 'p4RutgersRequestArtifacts') -eq 0) 'P4 validation record request artifact count is not zero'
    Assert-True ([int](Get-Property $record 'networkEvidenceRequests') -eq 0) 'P4 validation record network evidence request count is not zero'
    Assert-True ([int](Get-Property $record 'productSourceMutations') -eq 0) 'P4 validation record product source mutation count is not zero'
    Assert-True ([int](Get-Property $record 'productionMutations') -eq 0) 'P4 validation record production mutation count is not zero'
    Assert-True ([int](Get-Property $record 'packageBuilds') -eq 0) 'P4 validation record package build count is not zero'
    Assert-True (-not [bool](Get-Property $record 'deploymentExecutionAuthorized')) 'P4 validation record authorizes deployment execution'
    $eligibility = Get-Property $record 'eligibility'
    Assert-True (-not [bool](Get-Property $eligibility 'p6')) 'P4 validation record incorrectly authorizes P6'
    Assert-True (-not [bool](Get-Property $eligibility 'p7')) 'P4 validation record incorrectly authorizes P7'

    if ($ValidationMode -eq 'Candidate') {
        Assert-True ((Get-Property $record 'recordStatus') -eq 'P4_PASS_CANDIDATE_AWAITING_VALIDATOR') 'P4 candidate recordStatus drifted'
        Assert-True ((Get-Property $record 'p4Gate') -eq 'P4_PASS_CANDIDATE') 'P4 candidate p4Gate drifted'
        Assert-True ($reviewGate -match '(?m)^p4_gate=P4_PASS_CANDIDATE\s*$') 'P4 candidate review gate marker drifted'
    }
    else {
        Assert-True ((Get-Property $record 'recordStatus') -eq 'P4_PASS') 'P4 final recordStatus is not P4_PASS'
        Assert-True ((Get-Property $record 'p4Gate') -eq 'P4_PASS') 'P4 final p4Gate is not P4_PASS'
        Assert-True ([bool](Get-Property $eligibility 'p5')) 'P4 final record does not authorize P5'
        Assert-True ($reviewGate -match '(?m)^p4_gate=P4_PASS\s*$') 'P4 final review gate marker drifted'
        Assert-True ($reviewGate -match '(?m)^p5_eligible=TRUE\s*$') 'P4 final review gate does not authorize P5'
    }
    Assert-True ($reviewGate -match '(?m)^p6_started=FALSE\s*$') 'P4 review gate incorrectly starts P6'
    Assert-True ($reviewGate -match '(?m)^p7_authorized=FALSE\s*$') 'P4 review gate incorrectly authorizes P7'
    Assert-True ($reviewGate -match '(?m)^p4_rutgers_request_artifacts=0\s*$') 'P4 review gate request artifact marker drifted'
    Assert-True ($reviewGate -match '(?m)^production_mutations=0\s*$') 'P4 review gate production mutation marker drifted'

    if ($failures.Count -gt 0) {
        foreach ($failure in $failures) { Write-Output "VALIDATION_FAILURE=$failure" }
        Write-Output 'p4_gate=FAIL'
        Write-Output 'p5_eligibility=FALSE'
        Write-Output "integrity_failures=$($failures.Count)"
        exit 1
    }

    if ($ValidationMode -eq 'Candidate') {
        Write-Output 'p4_gate=P4_PASS_CANDIDATE'
        Write-Output 'p5_eligibility=FALSE'
    }
    else {
        Write-Output 'p4_gate=P4_PASS'
        Write-Output 'p5_eligibility=TRUE'
    }
    Write-Output 'integrity_failures=0'
}
finally {
    Pop-Location
}
