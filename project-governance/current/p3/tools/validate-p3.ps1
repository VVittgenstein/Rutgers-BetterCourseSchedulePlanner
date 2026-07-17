param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path,
    [ValidateSet('Candidate','Final')]
    [string]$ValidationMode = 'Final'
)

$ErrorActionPreference = 'Stop'
$p3 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$failures = [System.Collections.Generic.List[string]]::new()

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { $failures.Add($Message) }
}

function Assert-SameSet([object[]]$Actual, [object[]]$Expected, [string]$Label) {
    $actualSet = @($Actual | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    $expectedSet = @($Expected | ForEach-Object { [string]$_ } | Sort-Object -Unique)
    $difference = @(Compare-Object -ReferenceObject $expectedSet -DifferenceObject $actualSet)
    $detail = (($difference | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ',')
    Assert-True ($difference.Count -eq 0) "$Label set mismatch: $detail"
}

function Get-JsonPropertyValue([object]$Object, [string]$Name) {
    $property = @($Object.PSObject.Properties | Where-Object { $_.Name -eq $Name })
    if ($property.Count -eq 1) { return $property[0].Value }
    return $null
}

function Read-Json([string]$RelativePath) {
    return Get-Content -LiteralPath (Join-Path $p3 $RelativePath) -Raw -Encoding utf8 | ConvertFrom-Json
}

function Get-Sha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
}

function Get-FilePrefixLinesSha256([string]$Path, [int]$LineCount) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $seen = 0
    $prefixLength = -1
    for ($index = 0; $index -lt $bytes.Length; $index++) {
        if ($bytes[$index] -eq 10) {
            $seen++
            if ($seen -eq $LineCount) {
                $prefixLength = $index + 1
                break
            }
        }
    }
    if ($prefixLength -lt 0) { throw "file has fewer than $LineCount LF-terminated lines: $Path" }
    $prefix = [byte[]]::new($prefixLength)
    [System.Array]::Copy($bytes, 0, $prefix, 0, $prefixLength)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return (($sha.ComputeHash($prefix) | ForEach-Object { $_.ToString('X2') }) -join '')
    }
    finally { $sha.Dispose() }
}

function Assert-Hash([string]$Path, [string]$Expected, [string]$Label) {
    Assert-True ((Get-Sha256 $Path) -eq $Expected) "$Label hash mismatch"
}

Push-Location $RepositoryRoot
try {
    $required = @(
        '00-p3-charter-and-preflight.md','01a-catalog-request-manifest.json',
        '01c-catalog-request-amendment-001.json','01e-catalog-request-amendment-002.json',
        '02-catalog-request-ledger.tsv','03-catalog-evidence-register.tsv',
        '04a-catalog-profile.json','04b-catalog-scope-summary.tsv',
        '07-local-oneclick-implementation-plan.md','08-p3-traceability-matrix.tsv',
        '09-p3-validation-and-freeze-gate.md','10a-open-request-manifest.json',
        '10c-open-request-stop-001.json','11-open-request-ledger.tsv',
        '12-open-evidence-register.tsv','13a-open-profile.json',
        '13b-open-scope-join-summary.tsv','13c-open-body-clusters.tsv',
        '13d-open-transition-summary.tsv','13e-open-value-shape.tsv',
        '17-p3-open-review-stop-gate.md','18a-open-review-decision-001.json',
        '19-open-cache-and-notification-latency-contract.md','20-p3-open-decision-gate.md',
        '21b-open-round2-resumption-amendment-001.json',
        '22a-open-round2-completion.md','22b-open-round2-completion.json',
        '23a-shared-open-final-contract.md','23b-shared-open-final-contract.json',
        'notebooks/p3_catalog_data_quality.ipynb',
        'reports/p3-catalog-data-quality-artifact.json'
    )
    foreach ($name in $required) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p3 $name)) "missing P3 artifact: $name"
    }

    # Catalog: 20-request manifest, one transparent technical retry, and one
    # approved Fall-D scope correction. Summer remains a six-scope sample.
    $catalogManifestPath = Join-Path $p3 '01a-catalog-request-manifest.json'
    $catalogA1Path = Join-Path $p3 '01c-catalog-request-amendment-001.json'
    $catalogManifest = Read-Json '01a-catalog-request-manifest.json'
    $catalogA1 = Read-Json '01c-catalog-request-amendment-001.json'
    $catalogA2 = Read-Json '01e-catalog-request-amendment-002.json'
    $catalogManifestHash = Get-Sha256 $catalogManifestPath
    $catalogA1Hash = Get-Sha256 $catalogA1Path
    Assert-True ($catalogManifest.status -eq 'FROZEN_BEFORE_FIRST_COURSES_REQUEST') 'Catalog manifest is not frozen'
    Assert-True (@($catalogManifest.requests).Count -eq 20) 'Catalog base request count must be 20'
    Assert-True ($catalogA1.status -eq 'FROZEN_TECHNICAL_CORRECTION') 'Catalog amendment 001 is not frozen'
    Assert-True ($catalogA1.baseManifestSha256 -eq $catalogManifestHash) 'Catalog amendment 001 base hash mismatch'
    Assert-True (@($catalogA1.replacementRequests).Count -eq 1 -and $catalogA1.replacementRequests[0].id -eq 'CAT-C001-R1') 'Catalog amendment 001 request drifted'
    Assert-True ($catalogA2.status -eq 'FROZEN_SCOPE_CORRECTION') 'Catalog amendment 002 is not frozen'
    Assert-True ($catalogA2.baseManifestSha256 -eq $catalogManifestHash -and $catalogA2.priorAmendmentSha256 -eq $catalogA1Hash) 'Catalog amendment 002 hash chain mismatch'
    Assert-True ($catalogA2.scopePolicy -eq 'FALL_CURRENT_ALL_15_PLUS_SUMMER_MAIN_ONLINE_6') 'Catalog scope policy drifted'
    Assert-True (@($catalogA2.additionalRequests).Count -eq 1 -and $catalogA2.additionalRequests[0].id -eq 'CAT-C021' -and $catalogA2.additionalRequests[0].campus -eq 'D') 'Catalog Fall-D correction drifted'

    $catalogLedger = @(Import-Csv -LiteralPath (Join-Path $p3 '02-catalog-request-ledger.tsv') -Delimiter "`t")
    $catalogSuccess = @($catalogLedger | Where-Object { $_.outcome -eq 'SUCCESS' })
    $catalogFailure = @($catalogLedger | Where-Object { $_.outcome -eq 'FAILURE' })
    $expectedCatalogIds = @($catalogManifest.requests.id) + @($catalogA1.replacementRequests.id) + @($catalogA2.additionalRequests.id)
    Assert-True ($catalogLedger.Count -eq 22) "Catalog ledger rows expected 22, got $($catalogLedger.Count)"
    Assert-SameSet @($catalogLedger.request_id) $expectedCatalogIds 'Catalog ledger IDs'
    Assert-True ($catalogSuccess.Count -eq 21 -and $catalogFailure.Count -eq 1) 'Catalog success/failure totals drifted'
    if ($catalogFailure.Count -eq 1) {
        Assert-True ($catalogFailure[0].request_id -eq 'CAT-C001' -and $catalogFailure[0].error_class -eq 'CLIENT_GZIP_NOT_DECODED') 'Catalog retained failure drifted'
    }
    foreach ($row in $catalogSuccess) {
        Assert-True ($row.http_status -eq '200' -and $row.body_sha256 -match '^[0-9A-F]{64}$') "Catalog success metadata invalid: $($row.request_id)"
        $rawPath = Join-Path $RepositoryRoot $row.raw_relative_path
        Assert-True (Test-Path -LiteralPath $rawPath) "Catalog raw body missing: $($row.request_id)"
        if (Test-Path -LiteralPath $rawPath) { Assert-Hash $rawPath $row.body_sha256 "Catalog raw $($row.request_id)" }
    }
    $catalogRegister = @(Import-Csv -LiteralPath (Join-Path $p3 '03-catalog-evidence-register.tsv') -Delimiter "`t")
    $catalogProfile = Read-Json '04a-catalog-profile.json'
    $catalogScopes = @(Import-Csv -LiteralPath (Join-Path $p3 '04b-catalog-scope-summary.tsv') -Delimiter "`t")
    Assert-True ($catalogRegister.Count -eq 21 -and (@($catalogRegister.evidence_id | Sort-Object -Unique)).Count -eq 21) 'Catalog evidence register drifted'
    Assert-SameSet @($catalogRegister.request_id) @($catalogSuccess.request_id) 'Catalog register IDs'
    Assert-True ([int]$catalogProfile.successPayloads -eq 21 -and @($catalogProfile.scopeSummary).Count -eq 21) 'Catalog profile scope totals drifted'
    Assert-True ($catalogScopes.Count -eq 21) 'Catalog scope summary must contain 21 rows'
    Assert-True ([int]$catalogProfile.counts.courses -eq 10629 -and [int]$catalogProfile.counts.sections -eq 22069 -and [int]$catalogProfile.counts.meetings -eq 30804) 'Catalog raw counts drifted'

    # Open: preserve the historical 21-row stop prefix and separately validate
    # the completed 42-row ledger and its frozen completion record.
    $openManifestPath = Join-Path $p3 '10a-open-request-manifest.json'
    $openStopPath = Join-Path $p3 '10c-open-request-stop-001.json'
    $openLedgerPath = Join-Path $p3 '11-open-request-ledger.tsv'
    $decisionPath = Join-Path $p3 '18a-open-review-decision-001.json'
    $latencyPath = Join-Path $p3 '19-open-cache-and-notification-latency-contract.md'
    $amendmentPath = Join-Path $p3 '21b-open-round2-resumption-amendment-001.json'
    $openManifest = Read-Json '10a-open-request-manifest.json'
    $openStop = Read-Json '10c-open-request-stop-001.json'
    $decision = Read-Json '18a-open-review-decision-001.json'
    $amendment = Read-Json '21b-open-round2-resumption-amendment-001.json'
    $completion = Read-Json '22b-open-round2-completion.json'
    $openContract = Read-Json '23b-shared-open-final-contract.json'
    $manifestHash = Get-Sha256 $openManifestPath
    $stopHash = Get-Sha256 $openStopPath
    $decisionHash = Get-Sha256 $decisionPath
    $latencyHash = Get-Sha256 $latencyPath
    $amendmentHash = Get-Sha256 $amendmentPath
    $finalLedgerHash = Get-Sha256 $openLedgerPath
    $round1PrefixHash = Get-FilePrefixLinesSha256 $openLedgerPath 22

    Assert-True ($manifestHash -eq '707705756EEA4D269EDD1822F8529453B1E8C7838E23B1FC843789379B947703') 'Open manifest hash drifted'
    Assert-True ($openManifest.status -eq 'FROZEN_BEFORE_FIRST_OPEN_REQUEST' -and @($openManifest.requests).Count -eq 42) 'Open manifest status/count drifted'
    Assert-True ([int]$openManifest.policy.rounds -eq 2 -and [int]$openManifest.policy.hardAttemptLimit -eq 42) 'Open manifest round/budget drifted'
    Assert-True ([int]$openManifest.catalogReuse.catalogRequestsPermitted -eq 0) 'Open run may not request Catalog data'

    Assert-True ($openStop.status -eq 'FROZEN_SEMANTIC_STOP_AFTER_ROUND_1') 'Historical Open stop status drifted'
    Assert-True ($openStop.manifestSha256 -eq $manifestHash -and $openStop.ledgerSha256AtStop -eq $round1PrefixHash) 'Historical Open stop hash chain drifted'
    Assert-True ([int]$openStop.completedAttempts -eq 21 -and [int]$openStop.completedRounds -eq 1 -and $openStop.reason -eq 'SAME_SCOPE_OPEN_JOIN_CONTRACT_CONFLICT') 'Historical Open stop facts drifted'
    Assert-SameSet @($openStop.cancelledRounds) @(2) 'Historical Open stop cancelled rounds'

    Assert-True ($amendment.status -eq 'FROZEN_ROUND_2_RESUMPTION_AFTER_USER_REVIEW') 'Open Round 2 amendment is not frozen'
    Assert-True ($amendment.hashChain.manifestSha256 -eq $manifestHash -and $amendment.hashChain.historicalStopSha256 -eq $stopHash) 'Open amendment manifest/stop chain drifted'
    Assert-True ($amendment.hashChain.ledgerSha256AtAmendmentFreeze -eq $round1PrefixHash) 'Open amendment ledger-prefix hash drifted'
    Assert-True ($amendment.hashChain.reviewDecisionSha256 -eq $decisionHash -and $amendment.hashChain.latencyContractSha256 -eq $latencyHash) 'Open amendment decision/latency chain drifted'
    Assert-SameSet @($amendment.authorizedRequestIds) @($openManifest.requests | Where-Object { [int]$_.round -eq 2 } | ForEach-Object { $_.id }) 'Open Round 2 authorized IDs'
    Assert-True ([int]$amendment.budget.catalogRequestsPermitted -eq 0 -and [int]$amendment.budget.additionalOpenRequestIdsPermitted -eq 0) 'Open amendment expanded network scope'

    $openLedger = @(Import-Csv -LiteralPath $openLedgerPath -Delimiter "`t")
    Assert-True ($openLedger.Count -eq 42) "Open ledger rows expected 42, got $($openLedger.Count)"
    Assert-True ((@($openLedger.request_id | Sort-Object -Unique)).Count -eq 42) 'Open request IDs are not unique'
    for ($i = 0; $i -lt [Math]::Min($openLedger.Count, @($openManifest.requests).Count); $i++) {
        $row = $openLedger[$i]
        $request = $openManifest.requests[$i]
        Assert-True ($row.request_id -eq $request.id -and [int]$row.round -eq [int]$request.round -and [int]$row.round_ordinal -eq [int]$request.roundOrdinal) "Open ledger order/round drifted at row $($i + 1)"
        Assert-True ($row.term_id -eq $request.termId -and $row.campus -eq $request.campus) "Open ledger scope drifted: $($row.request_id)"
        Assert-True ($row.outcome -eq 'SUCCESS' -and $row.http_status -eq '200') "Open request not successful: $($row.request_id)"
        Assert-True ($row.cache_control -eq 'max-age=30') "Open Cache-Control drifted: $($row.request_id)"
        Assert-True ([int]$row.numeric_count -eq 0 -and [int]$row.string_count -eq [int]$row.top_level_rows -and [int]$row.duplicate_raw_values -eq 0) "Open value shape invalid: $($row.request_id)"
        $rawPath = Join-Path $RepositoryRoot $row.raw_relative_path
        Assert-True (Test-Path -LiteralPath $rawPath) "Open raw body missing: $($row.request_id)"
        if (Test-Path -LiteralPath $rawPath) { Assert-Hash $rawPath $row.body_sha256 "Open raw $($row.request_id)" }
    }
    $openRound1 = @($openLedger | Where-Object { $_.round -eq '1' })
    $openRound2 = @($openLedger | Where-Object { $_.round -eq '2' })
    Assert-True ($openRound1.Count -eq 21 -and $openRound2.Count -eq 21) 'Open per-round totals drifted'

    Assert-True ($completion.status -eq 'FROZEN_ROUND_2_COMPLETE') 'Open completion record is not frozen'
    Assert-True ([int]$completion.attempts.planned -eq 42 -and [int]$completion.attempts.completed -eq 42 -and [int]$completion.attempts.successful -eq 42 -and [int]$completion.attempts.failed -eq 0) 'Open completion attempt totals drifted'
    Assert-True ([int]$completion.furtherP3EvidenceRequestsAuthorized -eq 0) 'Further P3 Open requests were authorized'
    Assert-True ($completion.hashes.round2AmendmentSha256 -eq $amendmentHash -and $completion.hashes.finalLedgerSha256 -eq $finalLedgerHash) 'Open completion primary hashes drifted'
    $completionHashPaths = @{
        evidenceRegisterSha256='12-open-evidence-register.tsv'; openProfileSha256='13a-open-profile.json';
        scopeJoinSummarySha256='13b-open-scope-join-summary.tsv'; bodyClustersSha256='13c-open-body-clusters.tsv';
        transitionSummarySha256='13d-open-transition-summary.tsv'; valueShapeSha256='13e-open-value-shape.tsv'
    }
    foreach ($entry in $completionHashPaths.GetEnumerator()) {
        $expected = Get-JsonPropertyValue $completion.hashes $entry.Key
        Assert-Hash (Join-Path $p3 $entry.Value) $expected "Open completion $($entry.Key)"
    }

    $openRegister = @(Import-Csv -LiteralPath (Join-Path $p3 '12-open-evidence-register.tsv') -Delimiter "`t")
    $openProfile = Read-Json '13a-open-profile.json'
    $openJoin = @(Import-Csv -LiteralPath (Join-Path $p3 '13b-open-scope-join-summary.tsv') -Delimiter "`t")
    $openClusters = @(Import-Csv -LiteralPath (Join-Path $p3 '13c-open-body-clusters.tsv') -Delimiter "`t")
    $openTransitions = @(Import-Csv -LiteralPath (Join-Path $p3 '13d-open-transition-summary.tsv') -Delimiter "`t")
    $openShape = @(Import-Csv -LiteralPath (Join-Path $p3 '13e-open-value-shape.tsv') -Delimiter "`t")
    Assert-True ($openRegister.Count -eq 42 -and (@($openRegister.evidence_id | Sort-Object -Unique)).Count -eq 42) 'Open evidence register drifted'
    Assert-SameSet @($openRegister.request_id) @($openLedger.request_id) 'Open register IDs'
    Assert-True ($openProfile.status -eq 'ROUND_2_COMPLETE_OFFICIAL_JOIN_VALIDATED' -and $openProfile.round2 -eq 'COMPLETE' -and [int]$openProfile.successfulAttempts -eq 42) 'Open profile completion status drifted'
    $profileStringCount = [int](Get-JsonPropertyValue $openProfile.valueTypeCounts 'string')
    $profileLength5Count = [int](Get-JsonPropertyValue $openProfile.stringLengthCounts '5')
    Assert-True ($profileStringCount -eq 302125 -and $profileLength5Count -eq 302125) 'Open five-digit string totals drifted'
    Assert-True ([int](Get-JsonPropertyValue $openProfile.cacheControlCounts 'max-age=30') -eq 42) 'Open max-age=30 total drifted'
    Assert-True ([int]$openProfile.totals.officialJoinPassObservations -eq 42 -and [int]$openProfile.totals.unsafeEmptyObservations -eq 0 -and [int]$openProfile.totals.emptyOpenObservations -eq 2) 'Open join/empty totals drifted'
    Assert-True ([int]$openProfile.totals.changedRawScopePairs -eq 14 -and [int]$openProfile.totals.changedEffectiveScopePairs -eq 3) 'Open transition totals drifted'
    Assert-True ([int]$openProfile.totals.effectiveAddedSummedNonDistinct -eq 3 -and [int]$openProfile.totals.effectiveRemovedSummedNonDistinct -eq 8) 'Open effective add/remove totals drifted'
    Assert-True ($openJoin.Count -eq 42 -and @($openJoin | Where-Object { $_.official_join_result -like 'PASS_*' }).Count -eq 42) 'Open join table drifted'
    Assert-True ($openClusters.Count -eq 10) 'Open body cluster row count drifted'
    Assert-True ($openTransitions.Count -eq 21 -and @($openTransitions | Where-Object { $_.body_changed -eq 'true' }).Count -eq 14) 'Open raw transition table drifted'
    Assert-True (@($openTransitions | Where-Object { [int]$_.effective_open_added_count + [int]$_.effective_open_removed_count -gt 0 }).Count -eq 3) 'Open effective transition table drifted'
    $shape = @{}
    foreach ($row in $openShape) { $shape[$row.metric] = [int64]$row.count }
    Assert-True ($shape['root_string_values'] -eq 302125 -and $shape['string_length_5'] -eq 302125 -and $shape['root_non_string_values'] -eq 0 -and $shape['string_length_not_5'] -eq 0) 'Open value-shape table drifted'

    # Approved decision and final shared Open contract.
    Assert-True ($decision.status -eq 'FROZEN_JOIN_AND_RATE_CLOCK_MAPPING' -and $decision.joinDecision.status -eq 'APPROVED') 'Open review decision drifted'
    Assert-True ($decision.proposedClockMapping.status -eq 'APPROVED_BY_USER' -and [int]$decision.proposedClockMapping.activeWatchTargetSeconds -eq 10) 'Two-clock approval drifted'
    Assert-True ([int]$decision.rateDecision.publicGeneralIntervalSeconds -eq 30 -and -not [bool]$decision.rateDecision.publicUserConfigurable) 'Public Open interval drifted'
    Assert-True ([int]$decision.rateDecision.localDefaultIntervalSeconds -eq 30 -and [int]$decision.rateDecision.localMinimumIntervalSeconds -eq 3 -and [int]$decision.rateDecision.localMaximumIntervalSeconds -eq 3600) 'Local Open interval drifted'
    Assert-True (-not [bool]$decision.rateDecision.hardSourceChangeToNotificationGuarantee) 'Unsupported hard notification guarantee introduced'

    $expectedContractStatus = if ($ValidationMode -eq 'Final') { 'FROZEN_P3_SHARED_OPEN_CONTRACT' } else { 'P3_SHARED_OPEN_CONTRACT_FREEZE_CANDIDATE' }
    Assert-True ($openContract.status -eq $expectedContractStatus) "Shared Open contract status expected $expectedContractStatus"
    Assert-True ([int]$openContract.evidence.attempts -eq 42 -and [int]$openContract.evidence.additionalRequestsAuthorized -eq 0) 'Shared Open contract evidence boundary drifted'
    Assert-True ($openContract.identity.openBatchKey -eq 'term_campus' -and -not [bool]$openContract.identity.crossCampusUnionForStateMutation -and [int]$openContract.identity.currentRequiredSourceCountPerBatch -eq 1) 'Open target identity/batch drifted'
    Assert-True ($openContract.identity.orphanHandling -eq 'AUDIT_IGNORE_NEVER_CREATE_SECTION') 'Open orphan handling drifted'
    Assert-True ($openContract.classification.emptyArrayWithNonemptyCatalog -eq 'UNSAFE_EMPTY_KEEP_LAST_KNOWN_GOOD' -and $openContract.classification.nonemptyZeroIntersection -eq 'UNSAFE_ZERO_INTERSECTION_KEEP_LAST_KNOWN_GOOD') 'Open unsafe classification drifted'
    Assert-True ([int]$openContract.scheduler.publicGeneralSeconds -eq 30 -and -not [bool]$openContract.scheduler.publicUserConfigurable) 'Contract public clock drifted'
    Assert-True ([int]$openContract.scheduler.localGeneralDefaultSeconds -eq 30 -and [int]$openContract.scheduler.localGeneralMinimumSeconds -eq 3 -and [int]$openContract.scheduler.localGeneralMaximumSeconds -eq 3600 -and [int]$openContract.scheduler.activeWatchTargetSeconds -eq 10) 'Contract local/watch clocks drifted'
    Assert-True ([bool]$openContract.scheduler.perTargetSingleFlight -and -not [bool]$openContract.scheduler.sameTargetWatchCountAmplifiesRequests -and [int]$openContract.scheduler.realOriginMaxConcurrency -eq 1 -and [bool]$openContract.scheduler.sharedCatalogOpenOriginLimiter) 'Contract single-flight/origin limiter drifted'
    Assert-True ($openContract.scheduler.priority -eq 'EARLIEST_ABSOLUTE_DUE_WATCH_ONLY_TIE_BREAK' -and -not [bool]$openContract.scheduler.starvationPermitted) 'Contract scheduling fairness drifted'
    Assert-True ([int]$openContract.transport.connectTimeoutSeconds -eq 5 -and [int]$openContract.transport.totalTimeoutSeconds -eq 15 -and [int]$openContract.transport.maxDecodedBytes -eq 10485760 -and [int]$openContract.transport.automaticImmediateRetries -eq 0) 'Open transport safety drifted'
    Assert-True (-not [bool]$openContract.transport.conditionalGet304 -and $openContract.transport.etagUse -eq 'AUDIT_ONLY') 'Open ETag/conditional contract drifted'
    Assert-True ([int]$openContract.backoff.maximumBackoffStepSeconds -eq 600 -and [bool]$openContract.backoff.totalRetryDelayMayExceedBackoffStepCap) 'Open backoff cap semantics drifted'
    Assert-True ($openContract.observations.attemptType -eq 'OpenPullAttempt' -and $openContract.observations.targetType -eq 'OpenRefreshObservation' -and $openContract.observations.sectionType -eq 'OpenObservation') 'Open observation layers drifted'
    Assert-True ($openContract.freshness.formula -eq 'last_valid_completed+2*requested_effective_interval+15s' -and $openContract.freshness.staleOrUnknownFilterResult -eq 'UNCERTAIN') 'Open freshness contract drifted'
    Assert-True ($openContract.catalogVersionRace.versionDriftAction -eq 'NO_LKG_UPDATE_NO_CLOSED_COALESCED_NORMAL_DUE') 'Open Catalog-race action drifted'
    Assert-True ([int]$openContract.notifications.maxAudibleDefault -eq 3 -and $null -eq $openContract.notifications.maxAudibleProductUpperLimit -and $openContract.notifications.maxAudibleAppliesTo -eq 'ONE_SHOT_ONLY') 'Max audible contract drifted'
    Assert-True ($openContract.notifications.capReachedAction -eq 'KEEP_WATCH_ACTIVE_SILENT' -and [bool]$openContract.notifications.continuousAckSuppressesSameEpisode -and [bool]$openContract.notifications.continuousRearmRequiresReliableClosedThenOpen) 'Audio episode contract drifted'
    Assert-True (-not [bool]$openContract.latency.hardRealChangeToNotificationThirtySeconds -and [int]$openContract.latency.validObservationToServerFanoutTargetSeconds -eq 1) 'Open latency boundary drifted'

    # P2-to-P3 traceability.
    $trace = @(Import-Csv -LiteralPath (Join-Path $p3 '08-p3-traceability-matrix.tsv') -Delimiter "`t")
    $openTrace = @($trace | Where-Object { $_.open_boundary -eq 'RESOLVED_P3_SHARED_OPEN_CONTRACT' })
    Assert-True ($trace.Count -eq 384 -and (@($trace.trace_id | Sort-Object -Unique)).Count -eq 384) 'P3 trace row/ID count drifted'
    Assert-True ($openTrace.Count -eq 68 -and @($openTrace | Where-Object { $_.trace_status -ne 'MAPPED_P3_OPEN_FROZEN' }).Count -eq 0) 'P3 Open trace rows drifted'
    Assert-True (@($openTrace | Where-Object { $_.p3_evidence_refs -notlike '*23b-shared-open-final-contract.json*' }).Count -eq 0) 'P3 Open trace lacks final contract reference'
    foreach ($group in @($trace | Group-Object source_path)) {
        $sourcePath = Join-Path $RepositoryRoot $group.Name
        Assert-True (Test-Path -LiteralPath $sourcePath) "trace source missing: $($group.Name)"
        if (Test-Path -LiteralPath $sourcePath) {
            $expectedHashes = @($group.Group.source_sha256 | Sort-Object -Unique)
            Assert-True ($expectedHashes.Count -eq 1 -and (Get-Sha256 $sourcePath) -eq $expectedHashes[0]) "trace source hash drifted: $($group.Name)"
        }
    }

    # Candidate and final state gates are intentionally distinct to avoid
    # declaring FROZEN before the candidate has passed this validator.
    $charterText = Get-Content -LiteralPath (Join-Path $p3 '00-p3-charter-and-preflight.md') -Raw -Encoding utf8
    $planText = Get-Content -LiteralPath (Join-Path $p3 '07-local-oneclick-implementation-plan.md') -Raw -Encoding utf8
    $contractText = Get-Content -LiteralPath (Join-Path $p3 '23a-shared-open-final-contract.md') -Raw -Encoding utf8
    $historicalGate = Get-Content -LiteralPath (Join-Path $p3 '17-p3-open-review-stop-gate.md') -Raw -Encoding utf8
    $openGate = Get-Content -LiteralPath (Join-Path $p3 '20-p3-open-decision-gate.md') -Raw -Encoding utf8
    $totalGate = Get-Content -LiteralPath (Join-Path $p3 '09-p3-validation-and-freeze-gate.md') -Raw -Encoding utf8
    $workflowPath = Join-Path $RepositoryRoot 'project-governance/current/single-mainline-delivery-workflow.md'
    Assert-True (Test-Path -LiteralPath $workflowPath) 'authoritative workflow missing for P6 Review storage amendment validation'
    $workflowText = if (Test-Path -LiteralPath $workflowPath) { Get-Content -LiteralPath $workflowPath -Raw -Encoding utf8 } else { '' }
    Assert-True ($historicalGate.Contains('Historical stop record') -and $historicalGate.Contains('P3_REVIEW_BLOCKED_OPEN_JOIN_AND_RATE')) 'Historical Open stop record was not preserved'
    Assert-True ($planText.Contains('NOT P7 AUTHORIZATION')) 'P3 plan lost the P7 authorization boundary'

    # P6 Review local-storage amendment: assert the approved positive contract
    # and reject the superseded two-file/user-profile/CWD/fallback topology.
    $planStorageMarkers = @(
        'storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001',
        'database_relative_path=data/rbcsp.sqlite',
        'path_anchor=RESOLVED_EXECUTABLE_DIRECTORY',
        'current_working_directory_semantics=IGNORED',
        'alternate_storage_fallback=FORBIDDEN',
        'pre_network_writability_gate=REQUIRED',
        'live_database_count=1',
        'logical_domains=OPERATIONAL,PERSONAL',
        'reset_scope=PERSONAL_TABLES_ONLY',
        'archive_database_payloads=FORBIDDEN',
        'first_run_sequence=WRITABILITY_SCHEMA_ONLY_THEN_RUTGERS',
        'upgrade_data_directory=PRESERVE',
        'uninstall_scope=DELETE_UNPACK_ROOT_INCLUDING_DATA',
        'backup_scope=FULL_RBCSP_DATABASE',
        'p7_authorized=FALSE'
    )
    foreach ($marker in $planStorageMarkers) {
        Assert-True ($planText.Contains($marker)) "P3 local storage contract marker missing: $marker"
    }
    foreach ($forbidden in @('LocalAppData/RBCSP','catalog.sqlite','user.sqlite','Windows Known Folder API')) {
        Assert-True (-not $planText.Contains($forbidden)) "P3 plan retained superseded local storage topology: $forbidden"
        Assert-True (-not $contractText.Contains($forbidden)) "P3 shared Open prose retained superseded local storage topology: $forbidden"
        Assert-True (-not $workflowText.Contains($forbidden)) "authoritative workflow retained superseded local storage topology: $forbidden"
    }
    foreach ($forbidden in @(
        'path_anchor=CURRENT_WORKING_DIRECTORY',
        'alternate_storage_fallback=ALLOWED',
        'archive_database_payloads=ALLOWED',
        'reset_scope=DELETE_DATABASE',
        'first_run_sequence=PRELOADED_DATA'
    )) {
        Assert-True (-not $planText.Contains($forbidden)) "P3 plan contains forbidden local storage contract: $forbidden"
    }
    Assert-True ($contractText.Contains('P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001') -and $contractText.Contains('<package-root>/data/rbcsp.sqlite') -and $contractText.Contains('P7 authorization: `FALSE`')) 'Shared Open prose lacks the non-semantic Windows storage amendment/P7 boundary'
    Assert-True ($contractText.Contains('changes neither the OpenBatch/join/empty/error/scheduler/observation/notification contract nor the frozen evidence represented by `23b`')) 'Shared Open prose does not preserve frozen Open semantics/23b evidence'
    Assert-True ($totalGate.Contains('local_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001') -and $totalGate.Contains('local_database=data/rbcsp.sqlite') -and $totalGate.Contains('p6_review_final_approval=FALSE') -and $totalGate.Contains('p7_authorized=FALSE')) 'P3 total gate lacks storage amendment or authorization hard stop'
    Assert-True ($workflowText.Contains('P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001') -and $workflowText.Contains('<package-root>/data/rbcsp.sqlite') -and $workflowText.Contains('P7 NOT AUTHORIZED')) 'authoritative workflow lacks local storage amendment/P7 hard stop'
    if ($ValidationMode -eq 'Candidate') {
        Assert-True ($charterText.Contains('P3 FREEZE CANDIDATE') -and $planText.Contains('P3 LOCAL ONE-CLICK PLAN FREEZE CANDIDATE')) 'P3 candidate plan markers missing'
        Assert-True ($contractText.Contains('P3_SHARED_OPEN_CONTRACT_FREEZE_CANDIDATE')) 'P3 candidate contract marker missing'
        Assert-True ($totalGate.Contains('p3_gate=P3_FREEZE_CANDIDATE_VALIDATION_PENDING') -and $openGate.Contains('P3_OPEN_FREEZE_CANDIDATE_VALIDATION_PENDING')) 'P3 candidate gate markers missing'
    }
    else {
        Assert-True ($charterText.Contains('P3 PASS') -and $charterText.Contains('FROZEN') -and $planText.Contains('P3_LOCAL_ONECLICK_PLAN_FROZEN')) 'P3 final charter/plan markers missing'
        Assert-True ($contractText.Contains('FROZEN_P3_SHARED_OPEN_CONTRACT')) 'P3 final contract marker missing'
        Assert-True ($totalGate.Contains('p3_gate=P3_PASS') -and $totalGate.Contains('p4_eligibility=TRUE')) 'P3 final total gate marker missing'
        Assert-True ($openGate.Contains('P3_OPEN_SUBGATE_PASS')) 'P3 final Open subgate marker missing'

        $notebook = Read-Json 'notebooks/p3_catalog_data_quality.ipynb'
        $codeCells = @($notebook.cells | Where-Object { $_.cell_type -eq 'code' })
        Assert-True ($codeCells.Count -eq 7 -and @($codeCells | Where-Object { $null -eq $_.execution_count -or @($_.outputs).Count -eq 0 }).Count -eq 0) 'P3 notebook is not fully executed'
        $notebookText = Get-Content -LiteralPath (Join-Path $p3 'notebooks/p3_catalog_data_quality.ipynb') -Raw -Encoding utf8
        Assert-True ($notebookText.Contains('Open: 42/42 hashes verified') -and $notebookText.Contains('Clock mapping: public=30 fixed')) 'P3 notebook lacks final Open output'

        $report = Read-Json 'reports/p3-catalog-data-quality-artifact.json'
        Assert-True ($report.surface -eq 'report' -and $report.snapshot.status -eq 'ready') 'P3 report artifact is not ready'
        Assert-True (@($report.snapshot.datasets.scope_summary).Count -eq 21 -and @($report.snapshot.datasets.open_transitions).Count -eq 21) 'P3 report datasets drifted'
        Assert-True ($report.package_info.evidence_scope -eq 'P3_CATALOG_AND_TWO_ROUND_OPEN_COMPLETE_SHARED_CONTRACT_FROZEN') 'P3 report evidence scope drifted'
        $reportSourcePaths = @($report.manifest.sources | ForEach-Object { $_.path })
        foreach ($path in @('project-governance/current/p3/13a-open-profile.json','project-governance/current/p3/13d-open-transition-summary.tsv','project-governance/current/p3/18a-open-review-decision-001.json','project-governance/current/p3/22b-open-round2-completion.json','project-governance/current/p3/23b-shared-open-final-contract.json')) {
            Assert-True ($reportSourcePaths -contains $path) "P3 report source missing: $path"
        }
    }

    Write-Output "validation_mode=$ValidationMode"
    Write-Output "catalog_attempts=$($catalogLedger.Count)"
    Write-Output "catalog_successes=$($catalogSuccess.Count)"
    Write-Output "open_round1_successes=$($openRound1.Count)"
    Write-Output "open_round2_successes=$($openRound2.Count)"
    Write-Output "open_total_successes=$($openLedger.Count)"
    Write-Output "open_five_digit_string_values=$profileLength5Count"
    Write-Output "open_changed_raw_pairs=$($openProfile.totals.changedRawScopePairs)"
    Write-Output "open_changed_effective_pairs=$($openProfile.totals.changedEffectiveScopePairs)"
    Write-Output "trace_rows=$($trace.Count)"
    Write-Output "trace_open_rows=$($openTrace.Count)"
    Write-Output 'further_p3_network_requests_authorized=0'
    Write-Output 'local_storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001'
    Write-Output 'local_database=data/rbcsp.sqlite'
    Write-Output 'local_live_database_count=1'
    Write-Output 'local_storage_fallback=FORBIDDEN'
    if ($ValidationMode -eq 'Final') {
        Write-Output 'shared_open_contract=FROZEN_P3_SHARED_OPEN_CONTRACT'
        Write-Output 'local_plan=P3_LOCAL_ONECLICK_PLAN_FROZEN'
        Write-Output 'p3_gate=P3_PASS'
        Write-Output 'p4_eligibility=TRUE'
        Write-Output 'p7_authorized=FALSE'
    }
    else {
        Write-Output 'p3_gate=P3_FREEZE_CANDIDATE_VALIDATED'
        Write-Output 'p4_eligibility=FALSE'
    }
    Write-Output "integrity_failures=$($failures.Count)"
    if ($failures.Count) {
        foreach ($failureMessage in $failures) { Write-Output "FAIL: $failureMessage" }
        exit 1
    }
    Write-Output 'evidence_integrity=PASS'
    exit 0
}
finally {
    Pop-Location
}
