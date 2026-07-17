param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
)

$ErrorActionPreference = 'Stop'
$p2 = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$failures = [System.Collections.Generic.List[string]]::new()

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { $failures.Add($Message) }
}

Push-Location $RepositoryRoot
try {
    $required = @(
        '00-p2-charter-and-preflight.md',
        '01-file-universe.tsv',
        '01b-adjacent-surface-inventory.md',
        '02-file-semantic-matrix.md',
        '03-capability-all-matrix.md',
        '04-only-closure-matrix.md',
        '05-reuse-and-port-matrix.md',
        '06-filter-section-watch-contract.md',
        '07-p2-validation-and-review-gate.md'
    )
    foreach ($name in $required) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p2 $name)) "missing artifact: $name"
    }
    foreach ($toolName in @('generate-file-universe.ps1','validate-p2.ps1')) {
        Assert-True (Test-Path -LiteralPath (Join-Path $p2 "tools\$toolName")) "missing tool: $toolName"
    }

    $matrixPath = Join-Path $p2 '01-file-universe.tsv'
    $matrix = @(Import-Csv -LiteralPath $matrixPath -Delimiter "`t")
    Assert-True ($matrix.Count -eq 158) "file matrix row count expected 158, got $($matrix.Count)"
    Assert-True ((@($matrix.path | Sort-Object -Unique)).Count -eq 158) 'file matrix contains duplicate paths'
    Assert-True ((@($matrix.row_id | Sort-Object -Unique)).Count -eq 158) 'file matrix contains duplicate row IDs'

    $allowedCapabilities = @('REQUIRED','EXCLUDED','FUTURE','INTERNAL_ONLY','MIXED_SEE_02')
    $allowedDispositions = @('REUSE_AS_IS','REUSE_WITH_FIXES','EXTRACT','PORT','REWRITE','SPLIT','MERGE','REMOVE','DEFER','OUT_OF_SCOPE')
    $allowedOwnership = @('BASELINE_SHARED','LOCAL_ONLY','PUBLIC_DELTA','FUTURE','INTERNAL_TOOLING','HISTORICAL_EVIDENCE')
    foreach ($row in $matrix) {
        Assert-True ($row.capability -in $allowedCapabilities) "invalid capability: $($row.path)=$($row.capability)"
        Assert-True ($row.disposition -in $allowedDispositions) "invalid disposition: $($row.path)=$($row.disposition)"
        Assert-True ($row.ownership -in $allowedOwnership) "invalid ownership: $($row.path)=$($row.ownership)"
        Assert-True (Test-Path -LiteralPath $row.path) "matrix path missing: $($row.path)"
        if (Test-Path -LiteralPath $row.path) {
            $actualHash = (Get-FileHash -LiteralPath $row.path -Algorithm SHA256).Hash
            Assert-True ($actualHash -eq $row.sha256) "hash drift: $($row.path)"
        }
    }

    $semantic = Get-Content -LiteralPath (Join-Path $p2 '02-file-semantic-matrix.md') -Raw -Encoding utf8
    $mixed = @($matrix | Where-Object { $_.capability -eq 'MIXED_SEE_02' })
    Assert-True ($mixed.Count -eq 38) "mixed file count expected 38, got $($mixed.Count)"
    foreach ($row in $mixed) {
        $leaf = [IO.Path]::GetFileName($row.path)
        Assert-True ($semantic.Contains($leaf)) "mixed file lacks semantic anchor: $($row.path)"
    }

    $counts = [ordered]@{
        semantic = [regex]::Matches($semantic, '(?m)^\| SEM-').Count
        capability = [regex]::Matches((Get-Content -LiteralPath (Join-Path $p2 '03-capability-all-matrix.md') -Raw -Encoding utf8), '(?m)^\| CAP-').Count
        only = [regex]::Matches((Get-Content -LiteralPath (Join-Path $p2 '04-only-closure-matrix.md') -Raw -Encoding utf8), '(?m)^\| ONLY-').Count
        reuse = [regex]::Matches((Get-Content -LiteralPath (Join-Path $p2 '05-reuse-and-port-matrix.md') -Raw -Encoding utf8), '(?m)^\| R-').Count
        filter = [regex]::Matches((Get-Content -LiteralPath (Join-Path $p2 '06-filter-section-watch-contract.md') -Raw -Encoding utf8), '(?m)^\| FLT-').Count
    }
    Assert-True ($counts.semantic -eq 93) "semantic row count expected 93, got $($counts.semantic)"
    Assert-True ($counts.capability -eq 28) "capability row count expected 28, got $($counts.capability)"
    Assert-True ($counts.only -eq 24) "ONLY row count expected 24, got $($counts.only)"
    Assert-True ($counts.reuse -eq 59) "reuse row count expected 59, got $($counts.reuse)"
    Assert-True ($counts.filter -eq 22) "filter row count expected 22, got $($counts.filter)"

    $idSpecs = @(
        @{ Name = 'semantic'; Text = $semantic; Pattern = '(?m)^\| (SEM-[A-Z]+-\d+)' ; Expected = $counts.semantic },
        @{ Name = 'capability'; Text = (Get-Content -LiteralPath (Join-Path $p2 '03-capability-all-matrix.md') -Raw -Encoding utf8); Pattern = '(?m)^\| (CAP-(?:I)?\d+)' ; Expected = $counts.capability },
        @{ Name = 'only'; Text = (Get-Content -LiteralPath (Join-Path $p2 '04-only-closure-matrix.md') -Raw -Encoding utf8); Pattern = '(?m)^\| (ONLY-\d+)' ; Expected = $counts.only },
        @{ Name = 'reuse'; Text = (Get-Content -LiteralPath (Join-Path $p2 '05-reuse-and-port-matrix.md') -Raw -Encoding utf8); Pattern = '(?m)^\| (R-[A-Z]+-\d+)' ; Expected = $counts.reuse },
        @{ Name = 'filter'; Text = (Get-Content -LiteralPath (Join-Path $p2 '06-filter-section-watch-contract.md') -Raw -Encoding utf8); Pattern = '(?m)^\| (FLT-[CS]\d{2}[a-z]?)' ; Expected = $counts.filter }
    )
    foreach ($spec in $idSpecs) {
        $ids = @([regex]::Matches($spec.Text, $spec.Pattern) | ForEach-Object { $_.Groups[1].Value })
        Assert-True ($ids.Count -eq $spec.Expected) "$($spec.Name) ID parse count expected $($spec.Expected), got $($ids.Count)"
        Assert-True ((@($ids | Sort-Object -Unique)).Count -eq $ids.Count) "$($spec.Name) contains duplicate row IDs"
    }

    $semanticRows = @(Get-Content -LiteralPath (Join-Path $p2 '02-file-semantic-matrix.md') -Encoding utf8 | Where-Object { $_ -match '^\| SEM-' })
    $axisPattern = '^[RIFE] / `(REUSE_AS_IS|REUSE_WITH_FIXES|EXTRACT|PORT|REWRITE|SPLIT|MERGE|REMOVE|DEFER|OUT_OF_SCOPE)` / `(BASELINE_SHARED|LOCAL_ONLY|PUBLIC_DELTA|FUTURE|INTERNAL_TOOLING|HISTORICAL_EVIDENCE|HIST)`$'
    foreach ($row in $semanticRows) {
        $cells = @($row -split '\|')
        Assert-True ($cells.Count -ge 8) "malformed semantic row: $row"
        if ($cells.Count -ge 8) {
            Assert-True ($cells[5].Trim() -match $axisPattern) "invalid semantic three-axis cell: $($cells[1].Trim())=$($cells[5].Trim())"
        }
    }

    $decisionText = @(
        '02-file-semantic-matrix.md',
        '03-capability-all-matrix.md',
        '04-only-closure-matrix.md',
        '05-reuse-and-port-matrix.md',
        '06-filter-section-watch-contract.md'
    ) | ForEach-Object { Get-Content -LiteralPath (Join-Path $p2 $_) -Raw -Encoding utf8 }
    $joined = $decisionText -join "`n"
    Assert-True (-not [regex]::IsMatch($joined, '\b(P2_UNRESOLVED|UNCLASSIFIED|TBD|TODO|UNCLEAR)\b')) 'unresolved decision token remains'
    Assert-True (-not [regex]::IsMatch($joined, 'PORT_WITH_FIXES|EXTRACT_WITH_CONDITIONS|PORT_THEN_REMOVE|SPLIT\+|EXTRACT\+|PORT\+')) 'non-canonical disposition token remains'
    foreach ($requiredDecision in @(
        'Max audible notifications',
        'ONLINE + UNSPECIFIED',
        'OpenEpisode',
        'RefreshObservation',
        'FilterSchema',
        'Saved views manager',
        'appliedViewId',
        'en-US',
        'zh-CN',
        'Persistent history',
        'P2 Review'
    )) {
        Assert-True ($joined.Contains($requiredDecision)) "missing Review decision text: $requiredDecision"
    }

    $contract = Get-Content -LiteralPath (Join-Path $p2 '06-filter-section-watch-contract.md') -Raw -Encoding utf8
    $reviewGate = Get-Content -LiteralPath (Join-Path $p2 '07-p2-validation-and-review-gate.md') -Raw -Encoding utf8
    $workflow = Get-Content -LiteralPath (Join-Path $p2 '..\single-mainline-delivery-workflow.md') -Raw -Encoding utf8
    Assert-True ($semantic.Contains('OPEN/CLOSED/UNKNOWN')) 'shared status observation must carry OPEN/CLOSED/UNKNOWN'
    Assert-True ($semantic.Contains('SEM-DOC-013')) 'Share and Saved views semantic decisions must remain split'
    Assert-True ($semantic.Contains('browser filter URL restore')) 'public filter URL restore removal decision missing'
    Assert-True ($contract.Contains('per-target single-flight')) 'manual refresh amplification guard missing'
    Assert-True ([regex]::IsMatch($contract, 'Saved views.{0,120}REQUIRED / LOCAL_ONLY', 'IgnoreCase')) 'Saved views REQUIRED / LOCAL_ONLY contract missing'
    Assert-True ([regex]::IsMatch($contract, '(?im)^.*browser storage.*Saved views API.*saved-view codec/manager.*$')) 'public Saved views capability exclusion missing'
    Assert-True (-not [regex]::IsMatch(($joined + "`n" + $reviewGate + "`n" + $workflow), 'browser-profile|session-only', 'IgnoreCase')) 'obsolete public Saved views storage choice remains'
    Assert-True ([regex]::IsMatch($reviewGate, '(?im)^.*Saved views.*REQUIRED / LOCAL_ONLY.*API/storage/bundle.*$')) 'public Saved views surface denylist missing from Review gate'
    Assert-True (-not [regex]::IsMatch($reviewGate, 'Saved views.{0,120}(future|defer)', 'IgnoreCase')) 'Saved views still appears unresolved/future in Review gate'
    Assert-True ($reviewGate.Contains('P2 APPROVED')) 'P2 approval status missing'
    Assert-True ($reviewGate.Contains('P3 ELIGIBLE, NOT STARTED')) 'post-P2 stage boundary missing'

    $secretPatterns = @(
        '-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----',
        '\bgh[pousr]_[A-Za-z0-9]{30,}\b',
        '\bAKIA[0-9A-Z]{16}\b',
        '\bsk-[A-Za-z0-9_-]{20,}\b',
        '\bSG\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b'
    )
    $publicP2Text = Get-ChildItem -LiteralPath $p2 -File -Recurse | Where-Object { $_.Extension -in @('.md','.tsv','.ps1') } | ForEach-Object {
        Get-Content -LiteralPath $_.FullName -Raw -Encoding utf8
    }
    $publicJoined = $publicP2Text -join "`n"
    foreach ($pattern in $secretPatterns) {
        Assert-True (-not [regex]::IsMatch($publicJoined, $pattern)) "secret-like pattern found: $pattern"
    }

    Write-Output "file_rows=$($matrix.Count)"
    Write-Output "unique_paths=$((@($matrix.path | Sort-Object -Unique)).Count)"
    Write-Output "mixed_files=$($mixed.Count)"
    Write-Output "semantic_rows=$($counts.semantic)"
    Write-Output "capability_rows=$($counts.capability)"
    Write-Output "only_rows=$($counts.only)"
    Write-Output "reuse_rows=$($counts.reuse)"
    Write-Output "filter_rows=$($counts.filter)"
    Write-Output "failures=$($failures.Count)"
    if ($failures.Count) {
        $failures | ForEach-Object { Write-Error $_ }
        exit 1
    }
}
finally {
    Pop-Location
}
