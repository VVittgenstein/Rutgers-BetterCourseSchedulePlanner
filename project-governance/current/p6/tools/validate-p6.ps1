[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script:Failures = [System.Collections.Generic.List[string]]::new()

function Add-Failure {
    param([Parameter(Mandatory = $true)][string]$Message)
    $script:Failures.Add($Message)
}

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )
    if (-not $Condition) {
        Add-Failure $Message
    }
}

function Assert-TextMatch {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Text,
        [Parameter(Mandatory = $true)][string]$Pattern,
        [Parameter(Mandatory = $true)][string]$Message
    )
    Assert-True ([regex]::IsMatch($Text, $Pattern, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase -bor [System.Text.RegularExpressions.RegexOptions]::Multiline)) $Message
}

function Get-Text {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Add-Failure "missing required file: $Path"
        return ''
    }
    return [System.IO.File]::ReadAllText($Path, [System.Text.Encoding]::UTF8)
}

function Import-RequiredTsv {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string[]]$Columns
    )
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Add-Failure "missing required TSV: $Path"
        return @()
    }
    $header = [System.IO.File]::ReadLines($Path, [System.Text.Encoding]::UTF8) | Select-Object -First 1
    $actualColumns = @($header -split "`t", -1)
    foreach ($column in $Columns) {
        Assert-True ($actualColumns -contains $column) "TSV $Path is missing column '$column'"
    }
    return @((Import-Csv -LiteralPath $Path -Delimiter "`t" -Encoding UTF8))
}

function Split-Refs {
    param([AllowNull()][string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return @()
    }
    return @(
        $Value -split '[;,|]' |
            ForEach-Object { $_.Trim() } |
            Where-Object { $_ -and $_ -notmatch '^(NONE|N/A|NA|-)$' }
    )
}

function Get-RecursivePropertyValues {
    param(
        [AllowNull()]$Object,
        [Parameter(Mandatory = $true)][string[]]$Names
    )
    $result = [System.Collections.Generic.List[object]]::new()
    function Visit-Value {
        param([AllowNull()]$Value)
        if ($null -eq $Value -or $Value -is [string] -or $Value -is [ValueType]) {
            return
        }
        if ($Value -is [System.Collections.IDictionary]) {
            foreach ($key in $Value.Keys) {
                if ($Names -contains [string]$key) {
                    $result.Add($Value[$key])
                }
                Visit-Value $Value[$key]
            }
            return
        }
        if ($Value -is [System.Collections.IEnumerable]) {
            foreach ($item in $Value) {
                Visit-Value $item
            }
            return
        }
        foreach ($property in $Value.PSObject.Properties) {
            if ($Names -contains $property.Name) {
                $result.Add($property.Value)
            }
            Visit-Value $property.Value
        }
    }
    Visit-Value $Object
    return @($result)
}

function Assert-RecursiveValue {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string[]]$Names,
        [Parameter(Mandatory = $true)]$Expected,
        [Parameter(Mandatory = $true)][string]$Message
    )
    $values = @(Get-RecursivePropertyValues -Object $Object -Names $Names)
    $found = $false
    foreach ($value in $values) {
        if ([string]$value -eq [string]$Expected) {
            $found = $true
            break
        }
    }
    Assert-True $found $Message
}

function Assert-FrozenHashTable {
    param(
        [Parameter(Mandatory = $true)][string]$DocumentPath,
        [Parameter(Mandatory = $true)][string]$CurrentGovernanceRoot,
        [Parameter(Mandatory = $true)][string[]]$RequiredPhases,
        [Parameter(Mandatory = $true)][int]$MinimumEntries
    )
    $text = Get-Text $DocumentPath
    if (-not $text) {
        return
    }
    $matches = [regex]::Matches($text, '`(?<path>p[3-5]/[^`]+)`\s*\|\s*`(?<hash>[A-Fa-f0-9]{64})`')
    Assert-True ($matches.Count -ge $MinimumEntries) "$DocumentPath must freeze at least $MinimumEntries P3/P4/P5 input hashes"
    $seenPaths = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    $seenPhases = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($match in $matches) {
        $relative = $match.Groups['path'].Value.Replace('/', [System.IO.Path]::DirectorySeparatorChar)
        $expected = $match.Groups['hash'].Value.ToUpperInvariant()
        [void]$seenPhases.Add(($relative -split [regex]::Escape([string][System.IO.Path]::DirectorySeparatorChar))[0])
        Assert-True ($seenPaths.Add($relative)) "$DocumentPath repeats frozen input path $relative"
        $target = Join-Path $CurrentGovernanceRoot $relative
        if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
            Add-Failure "$DocumentPath freezes missing input $relative"
            continue
        }
        $actual = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash.ToUpperInvariant()
        Assert-True ($actual -eq $expected) "frozen hash drift for $relative (expected $expected, actual $actual)"
    }
    foreach ($phase in $RequiredPhases) {
        Assert-True ($seenPhases.Contains($phase)) "$DocumentPath must freeze at least one $phase input"
    }
}

$p6Root = Split-Path -Parent $PSScriptRoot
$currentRoot = Split-Path -Parent $p6Root
$governanceRoot = Split-Path -Parent $currentRoot
$workspaceRoot = Split-Path -Parent (Split-Path -Parent $governanceRoot)

$files = [ordered]@{
    charter = Join-Path $p6Root '00-p6-charter-and-preflight.md'
    localPlan = Join-Path $p6Root '01-final-local-oneclick-execution-plan.md'
    publicPlan = Join-Path $p6Root '02-final-public-package-and-deployment-execution-plan.md'
    dag = Join-Path $p6Root '03-shared-implementation-dependency-dag.md'
    tasks = Join-Path $p6Root '04-p7-task-and-commit-matrix.tsv'
    verification = Join-Path $p6Root '05-verification-capacity-packaging-release-plan.md'
    risks = Join-Path $p6Root '06-dependency-license-cleanup-risk.tsv'
    authorization = Join-Path $p6Root '07-release-and-production-authorization-boundary.md'
    trace = Join-Path $p6Root '08-p6-traceability-matrix.tsv'
    gate = Join-Path $p6Root '09-p6-review-gate.md'
    gateJson = Join-Path $p6Root '09a-p6-review-gate.json'
    decision = Join-Path $p6Root '10-p6-review-decision-record.md'
    decisionJson = Join-Path $p6Root '10a-p6-review-decision-record.json'
}

foreach ($path in $files.Values) {
    Assert-True (Test-Path -LiteralPath $path -PathType Leaf) "required P6 artifact does not exist: $path"
}

# P6 is a plan-only phase. Its artifact tree may contain Markdown, TSV, JSON, and this validator only.
$forbiddenFiles = @(
    Get-ChildItem -LiteralPath $p6Root -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Extension -match '^\.(rs|toml|lock|ts|tsx|js|jsx|css|scss|sql|exe|dll|msi|zip|7z|tar|gz|deb|rpm)$'
        }
)
$forbiddenFileNames = @($forbiddenFiles | ForEach-Object { $_.FullName }) -join ', '
Assert-True ($forbiddenFiles.Count -eq 0) "P6 contains product source, dependency, build, or package artifacts: $forbiddenFileNames"

$p3GatePath = Join-Path $currentRoot 'p3/09-p3-validation-and-freeze-gate.md'
$p4GatePath = Join-Path $currentRoot 'p4/07a-p4-validation-gate.json'
$p5GatePath = Join-Path $currentRoot 'p5/07a-p5-validation-gate.json'
$workflowPath = Join-Path $currentRoot 'single-mainline-delivery-workflow.md'
$p3GateText = Get-Text $p3GatePath
$workflowText = Get-Text $workflowPath
Assert-TextMatch $p3GateText '(?m)^p3_gate=P3_PASS\s*$' 'P3 input gate is not P3_PASS'
Assert-TextMatch $workflowText 'P3 PASS / P4 PASS / P5 PASS / P6 REVIEW READY / P7 NOT AUTHORIZED' 'single-mainline workflow does not record the current P6 Review stop state'
Assert-TextMatch $workflowText 'P6_REVIEW_READY / P7 NOT_STARTED_AWAITING_USER_APPROVAL / PRODUCTION NOT_AUTHORIZED' 'single-mainline workflow P6 Review section is stale or incomplete'
Assert-TextMatch $workflowText '\| P7\.5 \|' 'single-mainline workflow does not contain the independent P7.5 phase'
Assert-TextMatch $workflowText 'P7\.4 -> P7\.5 -> GitHub Release' 'single-mainline workflow does not place P7.5 before Release'

$p4Gate = $null
$p5Gate = $null
try {
    $p4Gate = (Get-Text $p4GatePath) | ConvertFrom-Json
} catch {
    Add-Failure "P4 gate JSON is invalid: $($_.Exception.Message)"
}
try {
    $p5Gate = (Get-Text $p5GatePath) | ConvertFrom-Json
} catch {
    Add-Failure "P5 gate JSON is invalid: $($_.Exception.Message)"
}
if ($null -ne $p4Gate) {
    Assert-True ([string]$p4Gate.p4Gate -eq 'P4_PASS') 'P4 input gate is not P4_PASS'
    Assert-True ([string]$p4Gate.p3InputGate -eq 'P3_PASS') 'P4 does not record P3_PASS input'
    Assert-True (-not [bool]$p4Gate.eligibility.p7) 'P4 must not authorize P7'
}
if ($null -ne $p5Gate) {
    Assert-True ([string]$p5Gate.p5Gate -eq 'P5_PASS') 'P5 input gate is not P5_PASS'
    Assert-True ([string]$p5Gate.p4InputGate -eq 'P4_PASS') 'P5 does not record P4_PASS input'
    Assert-True ([bool]$p5Gate.eligibility.p6) 'P5 does not grant P6 eligibility'
    Assert-True (-not [bool]$p5Gate.eligibility.p7) 'P5 must not authorize P7'
}

# Recalculate every frozen upstream hash chain, then P6's own direct P3/P4/P5 inputs.
Assert-FrozenHashTable -DocumentPath (Join-Path $currentRoot 'p4/00-p4-charter-and-preflight.md') -CurrentGovernanceRoot $currentRoot -RequiredPhases @('p3') -MinimumEntries 8
Assert-FrozenHashTable -DocumentPath (Join-Path $currentRoot 'p5/00-p5-charter-and-preflight.md') -CurrentGovernanceRoot $currentRoot -RequiredPhases @('p3', 'p4') -MinimumEntries 13
Assert-FrozenHashTable -DocumentPath $files.charter -CurrentGovernanceRoot $currentRoot -RequiredPhases @('p3', 'p4', 'p5') -MinimumEntries 5

$charterText = Get-Text $files.charter
$localText = Get-Text $files.localPlan
$publicText = Get-Text $files.publicPlan
$dagText = Get-Text $files.dag
$verificationText = Get-Text $files.verification
$authorizationText = Get-Text $files.authorization
$gateText = Get-Text $files.gate
$decisionText = Get-Text $files.decision

Assert-TextMatch $charterText '(?m)^status=P6_EXECUTION_PLAN_AMENDED_FOR_REVIEW\s*$' 'P6 charter status is not the amended Review state'
Assert-True (
    [regex]::IsMatch($charterText, '(?m)^p3/p4/p5_input_gate=PASS\s*$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase) -or
    (
        [regex]::IsMatch($charterText, '(?m)^p3_input_gate=P3_PASS\s*$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase) -and
        [regex]::IsMatch($charterText, '(?m)^p4_input_gate=P4_PASS\s*$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase) -and
        [regex]::IsMatch($charterText, '(?m)^p5_input_gate=P5_PASS\s*$', [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
    )
) 'P6 charter must record P3/P4/P5 input gates as PASS'
Assert-TextMatch $charterText '(?m)^p7_authorized=FALSE\s*$' 'P6 charter must keep P7 unauthorized'
Assert-TextMatch $charterText '(?m)^final_package_count=2\s*$' 'P6 charter must freeze exactly two final packages'
foreach ($marker in @(
    'rutgers_requests',
    'product_source_mutations',
    'dependency_mutations',
    'database_mutations',
    'product_builds',
    'package_builds',
    'release_publications',
    'production_mutations',
    'git_mutations'
)) {
    Assert-TextMatch $charterText "(?m)^$marker=0\s*$" "P6 charter must record $marker=0"
}
Assert-TextMatch $charterText '(?m)^external_read_only_preflight_performed=TRUE\s*$' 'P6 charter must record the user-authorized read-only Vultr preflight'
Assert-TextMatch $charterText '(?m)^vultr_initial_system_state=DEGRADED\s*$' 'P6 charter must preserve the initial Vultr degraded state'
Assert-TextMatch $charterText '(?m)^vultr_initial_failed_units=2\s*$' 'P6 charter must preserve the initial two failed units'
Assert-TextMatch $charterText '(?m)^vultr_initial_blocking_reason=FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH\s*$' 'P6 charter must preserve the initial fwupd blocker reason'
Assert-TextMatch $charterText '(?m)^vultr_baseline_remediation_status=COMPLETED_PASS\s*$' 'P6 charter must record the completed baseline remediation'
Assert-TextMatch $charterText '(?m)^vultr_current_system_state=RUNNING\s*$' 'P6 charter must record the current running system state'
Assert-TextMatch $charterText '(?m)^vultr_current_failed_units=0\s*$' 'P6 charter must record zero current failed units'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_initial_service_count=1\s*$' 'P6 charter must preserve the initial residual needrestart service count'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_service=unattended-upgrades\.service\s*$' 'P6 charter must name the residual unattended-upgrades service'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_kernel_status=1\s*$' 'P6 charter must retain needrestart kernel status 1'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_diagnostic_mode=NEEDRESTART_VERBOSE_LIST\s*$' 'P6 charter must record verbose needrestart diagnostic mode'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_reason_code=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY\s*$' 'P6 charter must record the residual obsolete Python reason code'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_obsolete_binary=/usr/bin/python3\.12\s*$' 'P6 charter must record the obsolete Python binary path'
Assert-TextMatch $charterText '(?m)^vultr_residual_causal_attribution=NOT_ASSERTED\s*$' 'P6 charter must avoid unsupported residual causality'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_safety_apt_daily=INACTIVE\s*$' 'P6 charter must record apt-daily inactive before restart'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_safety_apt_daily_upgrade=INACTIVE\s*$' 'P6 charter must record apt-daily-upgrade inactive before restart'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_safety_package_locks=NONE\s*$' 'P6 charter must record no apt/dpkg locks before restart'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_safety_dpkg_audit_lines=0\s*$' 'P6 charter must record clean dpkg audit before restart'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_authorized=TRUE\s*$' 'P6 charter must record unattended restart authorization'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_performed=TRUE\s*$' 'P6 charter must record unattended restart execution'
Assert-TextMatch $charterText '(?m)^vultr_unattended_restart_waiver=FALSE\s*$' 'P6 charter must record no waiver'
Assert-TextMatch $charterText '(?m)^vultr_residual_needrestart_cleared=TRUE\s*$' 'P6 charter must record the residual as cleared'
Assert-TextMatch $charterText '(?m)^vultr_current_needrestart_service_count=0\s*$' 'P6 charter must record zero current needrestart services'
Assert-TextMatch $charterText '(?m)^vultr_python_changed_by_bounded_mutations=FALSE\s*$' 'P6 charter must record no Python change'
Assert-TextMatch $charterText '(?m)^vultr_whole_machine_reboot_required=FALSE\s*$' 'P6 charter must record no whole-machine reboot requirement'
Assert-TextMatch $charterText '(?m)^vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK\s*$' 'P6 charter must record the healthy baseline and P7.5 recheck'
Assert-TextMatch $charterText '(?m)^vultr_remediation_events=2\s*$' 'P6 charter must record two bounded mutation events'
Assert-TextMatch $charterText '(?m)^vultr_remediation_transactions=4\s*$' 'P6 charter must record four bounded mutation transactions'
Assert-TextMatch $charterText '(?m)^vultr_mutations=2\s*$' 'P6 charter must record exactly two bounded Vultr mutation events'
Assert-TextMatch $charterText '(?m)^real_world_network_test_authorized=FALSE\s*$' 'P6 charter must keep live Rutgers tests unauthorized'
Assert-TextMatch $charterText '(?m)^vultr_staging_mutation_authorized=FALSE\s*$' 'P6 charter must keep Vultr staging mutations unauthorized'

Assert-TextMatch $localText '(?m)^status=P6_LOCAL_EXECUTION_PLAN_AMENDED_FOR_REVIEW\s*$' 'local execution plan is not amended for Review'
Assert-TextMatch $localText '(?m)^package=WINDOWS_LOCAL_RELEASE_ARCHIVE\s*$' 'local execution plan must produce the Windows local release archive'
foreach ($anchor in @('Windows', 'Saved views', 'Reset', '3', '3600', '30', '10')) {
    Assert-True ($localText.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "local execution plan is missing anchor '$anchor'"
}
foreach ($marker in @(
    'local_storage_root=PACKAGE_ROOT',
    'local_storage_anchor=EXECUTABLE_PACKAGE_ROOT',
    'local_primary_sqlite_relative_path=data/rbcsp.sqlite',
    'local_primary_sqlite_count=1',
    'local_operational_personal_tables_co_resident=TRUE',
    'local_storage_cwd_dependent=FALSE',
    'local_storage_fallback=NONE',
    'local_first_start_creates_database=TRUE',
    'local_release_contains_database=FALSE',
    'local_release_contains_real_catalog_open_data=FALSE'
)) {
    Assert-TextMatch $localText "(?m)^$([regex]::Escape($marker))\s*$" "local execution plan is missing storage marker $marker"
}
Assert-True ($localText -notmatch '(?i)catalog\.sqlite.*user\.sqlite|Known Folder API|LocalAppData/RBCSP') 'local execution plan still contains the superseded Known-Folder/two-database contract'

Assert-TextMatch $publicText '(?m)^status=P6_PUBLIC_EXECUTION_PLAN_AMENDED_FOR_REVIEW\s*$' 'public execution plan is not amended for Review'
Assert-TextMatch $publicText '(?m)^package=LINUX_PUBLIC_DEPLOYMENT_PACKAGE\s*$' 'public execution plan must produce the Linux public deployment package'
Assert-TextMatch $publicText '(?m)^deployment_is_third_package=FALSE\s*$' 'deployment must not be treated as a third package'
foreach ($anchor in @('Linux', 'systemd', 'Caddy', 'ephemeral', 'Saved views', '600', '30', '10')) {
    Assert-True ($publicText.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "public execution plan is missing anchor '$anchor'"
}
foreach ($marker in @(
    'public_state_root=/var/lib/bcsp',
    'public_first_start_creates_database=TRUE',
    'public_release_contains_database=FALSE',
    'public_release_contains_real_catalog_open_data=FALSE',
    'public_local_personal_table_count=0'
)) {
    Assert-TextMatch $publicText "(?m)^$([regex]::Escape($marker))\s*$" "public execution plan is missing storage marker $marker"
}

Assert-TextMatch $dagText '(?m)^status=P6_IMPLEMENTATION_DAG_FROZEN_FOR_REVIEW\s*$' 'implementation DAG is not frozen for Review'
Assert-TextMatch $dagText '(?m)^dag_cycle_count=0\s*$' 'implementation DAG must declare zero cycles'
Assert-TextMatch $dagText '(?m)^task_node_count=32\s*$' 'implementation DAG must declare 32 task nodes'
Assert-TextMatch $dagText '(?m)^p7_order=P7\.1>P7\.2>P7\.3>P7\.4>P7\.5\s*$' 'implementation DAG must preserve the P7.1 to P7.5 order'
Assert-TextMatch $dagText '(?m)^p7_5_task_count=5\s*$' 'implementation DAG must declare five P7.5 tasks'
Assert-TextMatch $dagText '(?m)^p7_2_skills=\$industrial-brutalist-ui\+\$design-taste-frontend\s*$' 'P7.2 must freeze both required UI skills'
Assert-TextMatch $dagText '(?m)^p7_3_skill=\$emil-design-eng\s*$' 'P7.3 must freeze the Emil design skill'
Assert-TextMatch $dagText '\|\s*`P-07`\s*\|[^\r\n]*`P7\.1-014`' 'public execution view P-07 must map the public operations-assets task P7.1-014'

Assert-TextMatch $verificationText '(?m)^status=P6_VERIFICATION_RELEASE_PLAN_AMENDED_FOR_REVIEW\s*$' 'verification/release plan is not amended for Review'
Assert-TextMatch $verificationText '(?m)^fake_upstream_cadences_seconds=3,10,30,3600\s*$' 'fake-upstream cadence matrix must cover 3, 10, 30, and 3600 seconds'
Assert-TextMatch $verificationText '(?m)^origin_max_concurrency=1\s*$' 'capacity plan must enforce origin concurrency=1'
Assert-TextMatch $verificationText '(?m)^valid_observation_to_fanout_target_seconds=1\s*$' 'notification fanout target must be one second after a valid observation'
foreach ($anchor in @('SBOM', 'license', 'secret')) {
    Assert-True ($verificationText.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "verification/release plan is missing anchor '$anchor'"
}
Assert-TextMatch $verificationText 'fake[- ]upstream' 'verification/release plan must use a fake upstream for capacity and failure tests'
Assert-TextMatch $verificationText 'clean[- ]machine' 'verification/release plan must require clean-machine package verification'
Assert-TextMatch $verificationText 'SHA-?256' 'verification/release plan must require SHA-256 package verification'
Assert-TextMatch $verificationText '(?m)^real_world_e2e_required=TRUE\s*$' 'verification plan must require real-world E2E'
Assert-TextMatch $verificationText '(?m)^real_world_e2e_subphase=P7\.5\s*$' 'real-world E2E must be an independent P7.5 subphase'
Assert-TextMatch $verificationText '(?m)^candidate_hash_immutable_during_p7_5=TRUE\s*$' 'P7.5 must consume immutable candidate hashes'
Assert-TextMatch $verificationText '(?m)^vultr_staging_requires_separate_mutation_authorization=TRUE\s*$' 'Vultr staging mutation must have a separate authorization'
foreach ($anchor in @('workflow_dispatch', '2N+5', '480', 'live_environment_minimum_gap_minutes=15', 'LIVE_PRECONDITION_NOT_MET', 'GitHub Actions', 'Vultr staging', 'restore')) {
    Assert-True ($verificationText.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "verification/release plan is missing P7.5 anchor '$anchor'"
}

Assert-TextMatch $authorizationText '(?m)^status=P6_AUTHORIZATION_BOUNDARY_AMENDED_FOR_REVIEW\s*$' 'authorization boundary is not amended for Review'
Assert-TextMatch $authorizationText '(?m)^p7_authorized=FALSE\s*$' 'authorization boundary must not authorize P7'
Assert-TextMatch $authorizationText '(?m)^github_release_authorized=FALSE\s*$' 'GitHub Release must remain conditional and unauthorized'
Assert-TextMatch $authorizationText '(?m)^production_deployment_authorized=FALSE\s*$' 'production deployment must remain separately unauthorized'
Assert-TextMatch $authorizationText '(?m)^real_world_network_test_authorized=FALSE\s*$' 'authorization boundary must keep live network tests unauthorized'
Assert-TextMatch $authorizationText '(?m)^vultr_staging_mutation_authorized=FALSE\s*$' 'authorization boundary must keep staging mutations unauthorized'
Assert-TextMatch $authorizationText '(?m)^vultr_read_only_preflight_completed=TRUE\s*$' 'authorization boundary must record the completed read-only preflight'
Assert-TextMatch $authorizationText '(?m)^vultr_initial_system_state=DEGRADED\s*$' 'authorization boundary must retain the initial degraded state'
Assert-TextMatch $authorizationText '(?m)^vultr_initial_failed_units=2\s*$' 'authorization boundary must retain the initial two failed units'
Assert-TextMatch $authorizationText '(?m)^vultr_baseline_remediation_status=COMPLETED_PASS\s*$' 'authorization boundary must record the completed baseline remediation'
Assert-TextMatch $authorizationText '(?m)^vultr_current_system_state=RUNNING\s*$' 'authorization boundary must record the current running state'
Assert-TextMatch $authorizationText '(?m)^vultr_current_failed_units=0\s*$' 'authorization boundary must record zero current failed units'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_initial_service_count=1\s*$' 'authorization boundary must preserve the initial residual count'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_service=unattended-upgrades\.service\s*$' 'authorization boundary must name the residual service'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_kernel_status=1\s*$' 'authorization boundary must retain needrestart kernel status 1'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_diagnostic_mode=NEEDRESTART_VERBOSE_LIST\s*$' 'authorization boundary must record verbose needrestart diagnostic mode'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_reason_code=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY\s*$' 'authorization boundary must record the residual reason code'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_obsolete_binary=/usr/bin/python3\.12\s*$' 'authorization boundary must record the obsolete binary path'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_causal_attribution=NOT_ASSERTED\s*$' 'authorization boundary must avoid unsupported residual causality'
Assert-TextMatch $authorizationText '(?m)^vultr_unattended_restart_safety_gate=PASS\s*$' 'authorization boundary must record a passing restart safety gate'
Assert-TextMatch $authorizationText '(?m)^vultr_unattended_restart_authorized=TRUE\s*$' 'authorization boundary must record restart authorization'
Assert-TextMatch $authorizationText '(?m)^vultr_unattended_restart_performed=TRUE\s*$' 'authorization boundary must record restart execution'
Assert-TextMatch $authorizationText '(?m)^vultr_unattended_restart_waiver=FALSE\s*$' 'authorization boundary must record no waiver'
Assert-TextMatch $authorizationText '(?m)^vultr_residual_needrestart_cleared=TRUE\s*$' 'authorization boundary must record cleared residual'
Assert-TextMatch $authorizationText '(?m)^vultr_current_needrestart_service_count=0\s*$' 'authorization boundary must record zero current needrestart services'
Assert-TextMatch $authorizationText '(?m)^vultr_python_changed_by_bounded_mutations=FALSE\s*$' 'authorization boundary must record no Python change'
Assert-TextMatch $authorizationText '(?m)^vultr_whole_machine_reboot_required=FALSE\s*$' 'authorization boundary must record no whole-machine reboot requirement'
Assert-TextMatch $authorizationText '(?m)^vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK\s*$' 'authorization boundary must record healthy baseline with P7.5 recheck'
Assert-TextMatch $authorizationText '(?m)^vultr_remediation_events=2\s*$' 'authorization boundary must record two bounded events'
Assert-TextMatch $authorizationText '(?m)^vultr_remediation_transactions=4\s*$' 'authorization boundary must record four bounded transactions'
Assert-TextMatch $authorizationText '(?m)^vultr_mutations=2\s*$' 'authorization boundary must record exactly two bounded Vultr mutation events'
foreach ($anchor in @('GitHub Release', 'production', 'authorization')) {
    Assert-True ($authorizationText.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "authorization boundary is missing anchor '$anchor'"
}

$taskColumns = @('task_id', 'subphase', 'task_name', 'dependencies', 'inputs', 'outputs', 'mandatory_tests', 'package_impact', 'required_skills', 'commit_boundary', 'push_boundary', 'stop_gate')
$tasks = @(Import-RequiredTsv -Path $files.tasks -Columns $taskColumns)
Assert-True ($tasks.Count -gt 0) 'P7 task matrix must contain tasks'
$taskById = @{}
foreach ($row in $tasks) {
    $id = [string]$row.task_id
    Assert-True (-not [string]::IsNullOrWhiteSpace($id)) 'task matrix contains an empty task_id'
    if ($taskById.ContainsKey($id)) {
        Add-Failure "duplicate task_id: $id"
    } else {
        $taskById[$id] = $row
    }
    foreach ($column in @('subphase', 'task_name', 'inputs', 'outputs', 'mandatory_tests', 'package_impact', 'commit_boundary', 'push_boundary', 'stop_gate')) {
        Assert-True (-not [string]::IsNullOrWhiteSpace([string]$row.$column)) "task $id has empty $column"
    }
}

$indegree = @{}
$dependents = @{}
foreach ($id in $taskById.Keys) {
    $indegree[$id] = 0
    $dependents[$id] = [System.Collections.Generic.List[string]]::new()
}
foreach ($row in $tasks) {
    $id = [string]$row.task_id
    foreach ($dependency in @(Split-Refs ([string]$row.dependencies))) {
        if (-not $taskById.ContainsKey($dependency)) {
            Add-Failure "task $id references missing dependency $dependency"
            continue
        }
        if ($dependency -eq $id) {
            Add-Failure "task $id depends on itself"
            continue
        }
        $indegree[$id] = [int]$indegree[$id] + 1
        $dependents[$dependency].Add($id)
    }
}
$queue = [System.Collections.Generic.Queue[string]]::new()
foreach ($id in $taskById.Keys) {
    if ([int]$indegree[$id] -eq 0) {
        $queue.Enqueue($id)
    }
}
$visited = 0
while ($queue.Count -gt 0) {
    $id = $queue.Dequeue()
    $visited++
    foreach ($dependent in $dependents[$id]) {
        $indegree[$dependent] = [int]$indegree[$dependent] - 1
        if ([int]$indegree[$dependent] -eq 0) {
            $queue.Enqueue($dependent)
        }
    }
}
Assert-True ($visited -eq $taskById.Count) "P7 task dependency graph contains a cycle (visited $visited of $($taskById.Count))"

$p72 = @($tasks | Where-Object { [string]$_.subphase -eq 'P7.2' })
$p73 = @($tasks | Where-Object { [string]$_.subphase -eq 'P7.3' })
$p74 = @($tasks | Where-Object { [string]$_.subphase -eq 'P7.4' })
$p75 = @($tasks | Where-Object { [string]$_.subphase -eq 'P7.5' })
Assert-True ($tasks.Count -eq 32) "task matrix must contain exactly 32 tasks, found $($tasks.Count)"
Assert-True (@($tasks | Where-Object { [string]$_.subphase -eq 'P7.1' }).Count -eq 15) 'task matrix must contain 15 P7.1 tasks'
Assert-True ($p72.Count -gt 0) 'task matrix has no P7.2 task'
Assert-True ($p73.Count -gt 0) 'task matrix has no P7.3 task'
Assert-True ($p74.Count -gt 0) 'task matrix has no P7.4 task'
Assert-True ($p72.Count -eq 4) 'task matrix must contain four P7.2 tasks'
Assert-True ($p73.Count -eq 3) 'task matrix must contain three P7.3 tasks'
Assert-True ($p74.Count -eq 5) 'task matrix must contain five P7.4 tasks'
Assert-True ($p75.Count -eq 5) 'task matrix must contain five P7.5 tasks'
$p72Skills = (@($p72 | ForEach-Object { [string]$_.required_skills }) -join ';')
Assert-True ($p72Skills.Contains('$industrial-brutalist-ui')) 'P7.2 tasks do not require $industrial-brutalist-ui'
Assert-True ($p72Skills.Contains('$design-taste-frontend')) 'P7.2 tasks do not require $design-taste-frontend'
$p73Skills = (@($p73 | ForEach-Object { [string]$_.required_skills }) -join ';')
Assert-True ($p73Skills.Contains('$emil-design-eng')) 'P7.3 tasks do not require $emil-design-eng'

$p73DependsOnP72 = $false
$p73DependsOnVisualVerification = $false
foreach ($row in $p73) {
    foreach ($dependency in @(Split-Refs ([string]$row.dependencies))) {
        if ($taskById.ContainsKey($dependency)) {
            $dependencyRow = $taskById[$dependency]
            if ([string]$dependencyRow.subphase -eq 'P7.2') {
                $p73DependsOnP72 = $true
            }
            $dependencyText = @(
                [string]$dependencyRow.task_id,
                [string]$dependencyRow.task_name,
                [string]$dependencyRow.outputs,
                [string]$dependencyRow.mandatory_tests,
                [string]$dependencyRow.stop_gate
            ) -join ' '
            if ($dependencyText -match '(?i)(visual|screenshot|render)') {
                $p73DependsOnVisualVerification = $true
            }
        }
    }
}
Assert-True $p73DependsOnP72 'P7.3 does not depend on a P7.2 task'
Assert-True $p73DependsOnVisualVerification 'P7.3 does not depend on an explicit P7.2 visual-verification record'

$p72Commits = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
$p72Gates = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($row in $p72) {
    [void]$p72Commits.Add([string]$row.commit_boundary)
    [void]$p72Gates.Add([string]$row.stop_gate)
}
foreach ($row in $p73) {
    Assert-True (-not $p72Commits.Contains([string]$row.commit_boundary)) "P7.3 task $($row.task_id) shares a commit boundary with P7.2"
    Assert-True (-not $p72Gates.Contains([string]$row.stop_gate)) "P7.3 task $($row.task_id) shares a review record/stop gate with P7.2"
}

$p74Text = @($p74 | ForEach-Object { @($_.task_name, $_.outputs, $_.mandatory_tests, $_.package_impact, $_.stop_gate) -join ' ' }) -join ' '
Assert-True ($p74Text -match 'WINDOWS_LOCAL_RELEASE_ARCHIVE') 'P7.4 does not name the Windows local release archive'
Assert-True ($p74Text -match 'LINUX_PUBLIC_DEPLOYMENT_PACKAGE') 'P7.4 does not name the Linux public deployment package'
Assert-True ($p74Text -match '(?i)(production|production_deployment)(_authorized)?=false') 'P7.4 does not explicitly keep production deployment false'
Assert-True ($p74Text -notmatch '(?i)(third[_ -]?package|package[_ -]?3)\s*=\s*true') 'P7.4 attempts to create a third package'
Assert-True ($p74Text -match 'P7_5_ELIGIBLE') 'P7.4 must finish with a P7.5 candidate entry gate'
Assert-True ($p74Text -notmatch '(?i)final P7 completion record|P7_REAL_WORLD_E2E_PASS') 'P7.4 must not claim final P7 completion after P7.5 was added'
Assert-True ($p74Text -match '(?i)(no DB|database|DB/WAL/SHM).*(real Catalog|real-Catalog|real Catalog or Open data)') 'P7.4 must reject packaged databases and preloaded real Catalog/Open data'

$expectedP75Ids = @('P7.5-001','P7.5-002','P7.5-003','P7.5-004','P7.5-005')
foreach ($id in $expectedP75Ids) {
    Assert-True ($taskById.ContainsKey($id)) "task matrix is missing $id"
}
if ($taskById.ContainsKey('P7.5-001')) {
    $deps = @(Split-Refs ([string]$taskById['P7.5-001'].dependencies))
    Assert-True ($deps.Count -eq 1 -and $deps[0] -eq 'P7.4-005') 'P7.5-001 must depend only on the P7.4 candidate gate'
}
if ($taskById.ContainsKey('P7.5-002')) {
    Assert-True (@(Split-Refs ([string]$taskById['P7.5-002'].dependencies)) -contains 'P7.5-001') 'Windows real-world E2E must depend on P7.5-001'
}
if ($taskById.ContainsKey('P7.5-003')) {
    Assert-True (@(Split-Refs ([string]$taskById['P7.5-003'].dependencies)) -contains 'P7.5-002') 'Actions Linux tier must follow Windows real-world E2E'
}
if ($taskById.ContainsKey('P7.5-004')) {
    Assert-True (@(Split-Refs ([string]$taskById['P7.5-004'].dependencies)) -contains 'P7.5-003') 'Vultr staging tier must follow the Actions Linux tier'
}
if ($taskById.ContainsKey('P7.5-005')) {
    $finalDeps = @(Split-Refs ([string]$taskById['P7.5-005'].dependencies))
    foreach ($dependency in @('P7.5-002','P7.5-003','P7.5-004')) {
        Assert-True ($finalDeps -contains $dependency) "P7.5-005 must depend on $dependency"
    }
}
$p75Text = @($p75 | ForEach-Object { @($_.task_id, $_.task_name, $_.inputs, $_.outputs, $_.mandatory_tests, $_.package_impact, $_.commit_boundary, $_.stop_gate) -join ' ' }) -join ' '
foreach ($anchor in @('Clean Windows', 'GitHub Actions', 'Vultr staging', 'real Rutgers', 'Caddy', 'HTTPS/WSS', 'desktop/mobile', 'same-hash', 'restore', 'P7_REAL_WORLD_E2E_PASS')) {
    Assert-True ($p75Text.IndexOf($anchor, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) "P7.5 task matrix is missing anchor '$anchor'"
}
Assert-True ($p75Text -match '(?i)production=false') 'P7.5 must explicitly remain non-production'
Assert-True ($p75Text -match '(?i)staging mutation authorization') 'P7.5 must require separate Vultr staging mutation authorization'
Assert-True ($p75Text -match '(?i)(immutable|unchanged|same-hash|same hash)') 'P7.5 must keep candidate hashes immutable'
Assert-True ($p75Text -notmatch '(?i)(third[_ -]?package|package[_ -]?3)\s*=\s*true') 'P7.5 attempts to create a third package'

$riskColumns = @('risk_id', 'dependency_or_area', 'ecosystem', 'intended_role', 'license', 'license_source', 'status', 'risk', 'cleanup_or_alternative', 'p7_task_id', 'mechanical_resolution')
$risks = @(Import-RequiredTsv -Path $files.risks -Columns $riskColumns)
Assert-True ($risks.Count -gt 0) 'dependency/license/cleanup/risk matrix must contain rows'
$riskIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($row in $risks) {
    $riskId = [string]$row.risk_id
    Assert-True (-not [string]::IsNullOrWhiteSpace($riskId)) 'risk matrix contains an empty risk_id'
    Assert-True ($riskIds.Add($riskId)) "duplicate risk_id: $riskId"
    Assert-True (@('APPROVED', 'REJECTED') -contains ([string]$row.status).ToUpperInvariant()) "risk $riskId has invalid status '$($row.status)'"
    foreach ($column in @('dependency_or_area', 'ecosystem', 'intended_role', 'license', 'license_source', 'risk', 'cleanup_or_alternative', 'p7_task_id', 'mechanical_resolution')) {
        Assert-True (-not [string]::IsNullOrWhiteSpace([string]$row.$column)) "risk $riskId has empty $column"
    }
    $rowText = @($row.PSObject.Properties.Value) -join "`t"
    Assert-True ($rowText -notmatch '(?i)\b(UNKNOWN|UNAPPROVED)\b') "risk $riskId contains UNKNOWN or UNAPPROVED"
    foreach ($taskId in @(Split-Refs ([string]$row.p7_task_id))) {
        Assert-True ($taskById.ContainsKey($taskId)) "risk $riskId maps to missing P7 task $taskId"
    }
}
Assert-True ($risks.Count -eq 42) "risk matrix must retain 42 rows, found $($risks.Count)"
$approvedRisks = @($risks | Where-Object { ([string]$_.status).ToUpperInvariant() -eq 'APPROVED' })
$rejectedRisks = @($risks | Where-Object { ([string]$_.status).ToUpperInvariant() -eq 'REJECTED' })
Assert-True ($approvedRisks.Count -eq 29) "risk matrix must contain 29 APPROVED rows, found $($approvedRisks.Count)"
Assert-True ($rejectedRisks.Count -eq 13) "risk matrix must contain 13 REJECTED rows, found $($rejectedRisks.Count)"
$directoriesRisk = @($risks | Where-Object { [string]$_.risk_id -eq 'P6-D012' })
Assert-True ($directoriesRisk.Count -eq 1) 'risk matrix must contain exactly one P6-D012 row'
if ($directoriesRisk.Count -eq 1) {
    Assert-True ([string]$directoriesRisk[0].status -eq 'REJECTED') 'P6-D012 directories must be REJECTED after package-root storage approval'
    $directoriesText = @($directoriesRisk[0].PSObject.Properties.Value) -join ' '
    Assert-True ($directoriesText -match 'current_exe') 'P6-D012 must name executable-path resolution as the replacement'
    Assert-True ($directoriesText -match '(?i)No final consumer|NO_FINAL_CONSUMER') 'P6-D012 must have no final consumer'
}

$traceColumns = @('trace_id', 'source_phase', 'source_artifact', 'source_requirement', 'kind', 'p7_task_ids', 'verification_ids', 'package_impact', 'risk_ids', 'status')
$traceRows = @(Import-RequiredTsv -Path $files.trace -Columns $traceColumns)
Assert-True ($traceRows.Count -gt 0) 'P6 traceability matrix must contain rows'
$traceIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($row in $traceRows) {
    $traceId = [string]$row.trace_id
    Assert-True (-not [string]::IsNullOrWhiteSpace($traceId)) 'trace matrix contains an empty trace_id'
    Assert-True ($traceIds.Add($traceId)) "duplicate trace_id: $traceId"
    if (@('P3', 'P4', 'P5') -contains ([string]$row.source_phase).ToUpperInvariant()) {
        foreach ($column in @('source_artifact', 'source_requirement', 'kind', 'p7_task_ids', 'verification_ids', 'package_impact', 'risk_ids', 'status')) {
            Assert-True (-not [string]::IsNullOrWhiteSpace([string]$row.$column)) "trace $traceId has empty $column"
        }
        Assert-True ([string]$row.status -match '(?i)(MAPPED|TRACE_COMPLETE|COMPLETE)') "trace $traceId is not mapped to P7"
        foreach ($taskId in @(Split-Refs ([string]$row.p7_task_ids))) {
            Assert-True ($taskById.ContainsKey($taskId)) "trace $traceId maps to missing P7 task $taskId"
        }
        foreach ($riskId in @(Split-Refs ([string]$row.risk_ids))) {
            Assert-True ($riskIds.Contains($riskId)) "trace $traceId maps to missing risk $riskId"
        }
    }
}
foreach ($phase in @('P3', 'P4', 'P5')) {
    $phaseRows = @($traceRows | Where-Object { ([string]$_.source_phase).ToUpperInvariant() -eq $phase })
    Assert-True ($phaseRows.Count -gt 0) "trace matrix has no $phase source rows"
    $kinds = @($phaseRows | ForEach-Object { ([string]$_.kind).ToUpperInvariant() })
    Assert-True (($kinds -join ' ') -match 'CAPABILITY') "$phase trace coverage omits capabilities"
    Assert-True (($kinds -join ' ') -match '(TEST|VERIFICATION)') "$phase trace coverage omits tests/verifications"
    Assert-True (($kinds -join ' ') -match 'RISK') "$phase trace coverage omits risks"
    Assert-True (($kinds -join ' ') -match '(ARTIFACT|PACKAGE)') "$phase trace coverage omits artifacts/packages"
}
Assert-True ($traceRows.Count -eq 27) "P6 trace matrix must retain 27 upstream trace rows, found $($traceRows.Count)"
foreach ($phase in @('P3', 'P4', 'P5')) {
    Assert-True (@($traceRows | Where-Object { ([string]$_.source_phase).ToUpperInvariant() -eq $phase }).Count -eq 9) "$phase trace coverage must retain nine rows"
}
foreach ($p75Id in $expectedP75Ids) {
    $mapped = @($traceRows | Where-Object { @(Split-Refs ([string]$_.p7_task_ids)) -contains $p75Id })
    Assert-True ($mapped.Count -gt 0) "trace matrix does not map any upstream requirement to $p75Id"
}

$decisionJson = $null
try {
    $decisionJson = (Get-Text $files.decisionJson) | ConvertFrom-Json
} catch {
    Add-Failure "P6 review decision JSON is invalid: $($_.Exception.Message)"
}
if ($null -ne $decisionJson) {
    Assert-True ([string]$decisionJson.recordId -eq 'P6-REVIEW-DECISION-2026-07-13-001') '10a recordId is wrong'
    Assert-True ([string]$decisionJson.storageAmendmentId -eq 'P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001') '10a storage amendment ID is wrong'
    Assert-True ([bool]$decisionJson.authorizations.p7Plan) '10a must record the approved P7 plan'
    Assert-True ([bool]$decisionJson.authorizations.liveRequestBudget) '10a must record the approved live request budget'
    Assert-True (-not [bool]$decisionJson.authorizations.p7) '10a must not authorize P7'
    Assert-True (-not [bool]$decisionJson.authorizations.realWorldNetworkTest) '10a must not authorize live network tests'
    Assert-True (-not [bool]$decisionJson.authorizations.vultrStagingMutation) '10a must not authorize Vultr staging mutation'
    Assert-True (-not [bool]$decisionJson.authorizations.githubRelease) '10a must not authorize GitHub Release'
    Assert-True (-not [bool]$decisionJson.authorizations.production) '10a must not authorize production'
    Assert-True ([bool]$decisionJson.authorizations.vultrReadOnlyPreflight) '10a must record the authorized read-only Vultr preflight'
    Assert-True ([bool]$decisionJson.authorizations.vultrBaselineRemediation) '10a must authorize only the bounded Vultr baseline remediation'
    Assert-True ([bool]$decisionJson.authorizations.vultrUnattendedRestart) '10a must authorize the bounded unattended-upgrades restart'
    Assert-True ([int]$decisionJson.p7Plan.existingAcceptedTasks -eq 27) '10a must retain the accepted 27-task base'
    Assert-True ([int]$decisionJson.p7Plan.newP75TasksApproved -eq 5) '10a must approve five P7.5 tasks'
    Assert-True ([int]$decisionJson.p7Plan.totalTasksApproved -eq 32) '10a must approve 32 total P7 tasks'
    Assert-True ([string]$decisionJson.p7Plan.finalCompletionTask -eq 'P7.5-005') '10a final completion task must be P7.5-005'
    Assert-True ([int]$decisionJson.p7Plan.exactPackageCount -eq 2) '10a exact package count must be two'
    Assert-True ([bool]$decisionJson.liveBudgetApproved.approved) '10a live request budget must be formally approved'
    Assert-True ([string]$decisionJson.liveBudgetApproved.perEnvironmentAttemptFormula -eq '2N+5') '10a live request budget formula must be 2N+5'
    Assert-True ([int]$decisionJson.liveBudgetApproved.environmentWindowSeconds -eq 480) '10a live window must be 480 seconds'
    Assert-True ([int]$decisionJson.liveBudgetApproved.minimumGapMinutes -eq 15) '10a environment gap must be 15 minutes'
    Assert-True ([int]$decisionJson.vultrReadOnlyFinding.readOnlyPreflightMutations -eq 0) '10a must record zero mutations during the read-only preflight'
    Assert-True ([string]$decisionJson.vultrReadOnlyFinding.initialSystemState -eq 'DEGRADED') '10a must retain the initial degraded system state'
    Assert-True ([int]$decisionJson.vultrReadOnlyFinding.initialFailedUnits -eq 2) '10a must retain the initial two failed units'
    Assert-True ([string]$decisionJson.vultrReadOnlyFinding.reasonCode -eq 'FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH') '10a must retain the fwupd blocker reason code'
    Assert-True ([bool]$decisionJson.vultrReadOnlyFinding.remediationAuthorized) '10a must record baseline remediation authorization'
    Assert-True ([string]$decisionJson.vultrReadOnlyFinding.remediationExecutionStatus -eq 'COMPLETED_PASS') '10a must record the completed baseline remediation'
    Assert-True ([string]$decisionJson.vultrReadOnlyFinding.currentSystemState -eq 'RUNNING') '10a must record the current running system state'
    Assert-True ([int]$decisionJson.vultrReadOnlyFinding.currentFailedUnits -eq 0) '10a must record zero current failed units'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationAuthorization.authorized) '10a bounded baseline remediation must be authorized'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationAuthorization.executionStatus -eq 'COMPLETED_PASS') '10a bounded remediation must be recorded as completed and passing'
    Assert-True (@($decisionJson.vultrBaselineRemediationAuthorization.exactInstallPackages).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationAuthorization.exactInstallPackages[0] -eq 'libfwupd3') '10a remediation install allowlist must contain only libfwupd3'
    Assert-True (@($decisionJson.vultrBaselineRemediationAuthorization.exactUpgradePackages).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationAuthorization.exactUpgradePackages[0] -eq 'fwupd') '10a remediation upgrade allowlist must contain only fwupd'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationAuthorization.maximumRemovePackages -eq 0) '10a remediation must allow zero package removals'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationAuthorization.stopBeforeMutationIfDiffExpands) '10a remediation must stop before mutation when the diff expands'
    Assert-True (@($decisionJson.vultrBaselineRemediationAuthorization.forbiddenOperations) -contains 'CREATE_SNAPSHOT') '10a remediation must forbid snapshots'
    Assert-True (@($decisionJson.vultrBaselineRemediationAuthorization.forbiddenOperations) -contains 'WHOLE_MACHINE_REBOOT') '10a remediation must forbid whole-machine reboot'
    Assert-True (@($decisionJson.vultrBaselineRemediationAuthorization.forbiddenOperations) -contains 'UPGRADE_OTHER_PACKAGES') '10a remediation must forbid other package upgrades'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationAuthorization.doesNotAuthorizeP7) '10a remediation must not authorize P7'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationAuthorization.doesNotAuthorizeRealWorldNetworkTest) '10a remediation must not authorize live network tests'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationAuthorization.doesNotAuthorizeVultrStagingMutation) '10a remediation must not authorize staging mutation'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.status -eq 'COMPLETED_PASS') '10a remediation result must pass'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.metadataRefresh -eq 'PASS') '10a metadata refresh must pass'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.diffMatchedAuthorization) '10a post-refresh simulation must match the authorization'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.installPackages).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.installPackages[0] -eq 'libfwupd3') '10a actual simulation install set must contain only libfwupd3'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.upgradePackages).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.upgradePackages[0] -eq 'fwupd') '10a actual simulation upgrade set must contain only fwupd'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.postRefreshSimulation.removePackages).Count -eq 0) '10a actual simulation must remove zero packages'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.currentSystemState -eq 'RUNNING') '10a remediation result must leave systemd running'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.currentFailedUnits -eq 0) '10a remediation result must leave zero failed units'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.fwupdServiceActiveState -eq 'active') '10a fwupd service must be active'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.fwupdServiceResult -eq 'success') '10a fwupd service result must be success'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.fwupdRefreshServiceResult -eq 'success') '10a fwupd-refresh result must be success'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.dpkgAuditLines -eq 0) '10a dpkg audit must be clean'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.heldPackages -eq 0) '10a must retain zero held packages'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.rebootRequired) '10a remediation must not require reboot'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.fwupdPendingChanges -eq 0) '10a fwupd must have zero pending changes'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.snapshotCreated) '10a remediation must not create a snapshot'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.wholeMachineRebooted) '10a remediation must not reboot the whole machine'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.bcspChanged) '10a remediation must not change BCSP'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.caddyChanged) '10a remediation must not change Caddy'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.remediationEvents -eq 1) '10a must record one remediation event'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.transactions.total -eq 3) '10a must record three remediation transactions'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceCount -eq 1) '10a must record one residual needrestart service'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.residualNeedrestart.services).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.services[0] -eq 'unattended-upgrades.service') '10a residual service must be unattended-upgrades.service only'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceActiveState -eq 'active') '10a residual service must be active'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceSubState -eq 'running') '10a residual service must be running'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceResult -eq 'success') '10a residual service result must be success'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.diagnosticMode -eq 'NEEDRESTART_VERBOSE_LIST') '10a must record verbose needrestart diagnostic mode'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.reasonCode -eq 'UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY') '10a must record the residual obsolete Python reason code'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.processRole -eq 'unattended-upgrade-shutdown') '10a must record the obsolete-binary process role without a PID'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.obsoleteBinary -eq '/usr/bin/python3.12') '10a must record the obsolete Python binary path'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.suggestedAction -eq 'systemctl restart unattended-upgrades.service') '10a must retain the needrestart suggested action'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.needrestartKernelStatus -eq 1) '10a must retain needrestart kernel status 1'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.wholeMachineRebootRequired) '10a must record no whole-machine reboot requirement'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.residualNeedrestart.authorizedPackageInstallSet).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.authorizedPackageInstallSet[0] -eq 'libfwupd3') '10a residual record must preserve the exact authorized install set'
    Assert-True (@($decisionJson.vultrBaselineRemediationResult.residualNeedrestart.authorizedPackageUpgradeSet).Count -eq 1 -and [string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.authorizedPackageUpgradeSet[0] -eq 'fwupd') '10a residual record must preserve the exact authorized upgrade set'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.pythonChangedByRemediation) '10a must record that remediation did not change Python'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.causalAttribution -eq 'NOT_ASSERTED') '10a must avoid unsupported residual causality'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceRestartAuthorized) '10a must record residual service restart authorization'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.serviceRestartPerformed) '10a must record the residual service restart'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.waiverGranted) '10a must record that no waiver was granted'
    Assert-True ([bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.residualCleared) '10a must record the residual as cleared'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.preRestartSafetyGate.status -eq 'PASS') '10a restart safety gate must pass'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.preRestartSafetyGate.aptDailyState -eq 'inactive') '10a apt-daily must be inactive before restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.preRestartSafetyGate.aptDailyUpgradeState -eq 'inactive') '10a apt-daily-upgrade must be inactive before restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.preRestartSafetyGate.aptDpkgLocks -eq 'none') '10a must record no apt/dpkg locks before restart'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.preRestartSafetyGate.dpkgAuditLines -eq 0) '10a pre-restart dpkg audit must be clean'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.executedAction -eq 'systemctl restart unattended-upgrades.service') '10a must record the sole restart action'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.systemState -eq 'RUNNING') '10a post-restart systemd must be running'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.failedUnits -eq 0) '10a post-restart failed units must be zero'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.serviceActiveState -eq 'active') '10a unattended-upgrades must be active after restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.serviceSubState -eq 'running') '10a unattended-upgrades must be running after restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.serviceResult -eq 'success') '10a unattended-upgrades result must be success after restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.fwupdServiceActiveState -eq 'active') '10a fwupd must remain active after restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.fwupdServiceResult -eq 'success') '10a fwupd result must remain success after restart'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.fwupdRefreshServiceResult -eq 'success') '10a fwupd-refresh result must remain success after restart'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.needrestartServiceCount -eq 0) '10a post-restart needrestart service count must be zero'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.dpkgAuditLines -eq 0) '10a post-restart dpkg audit must be clean'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.heldPackages -eq 0) '10a post-restart held packages must be zero'
    Assert-True (-not [bool]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.rebootRequired) '10a post-restart must not require reboot'
    Assert-True ([int]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.postRestart.fwupdPendingChanges -eq 0) '10a post-restart fwupd pending changes must be zero'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.stagingReadiness -eq 'BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK') '10a must record healthy staging baseline with recheck'
    Assert-True ([string]$decisionJson.vultrBaselineRemediationResult.residualNeedrestart.mustRecheckAtTask -eq 'P7.5-001') '10a must require residual recheck at P7.5-001'
    Assert-True ([string]$decisionJson.vultrStagingReadiness -eq 'BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK') '10a top-level staging readiness must be healthy with recheck'
    Assert-True ([int]$decisionJson.sideEffects.vultrMutations -eq 2) '10a must record exactly two bounded Vultr mutation events'
    Assert-True ([int]$decisionJson.sideEffects.vultrBaselineRemediationEvents -eq 1) '10a must record one bounded remediation event'
    Assert-True ([int]$decisionJson.sideEffects.vultrBaselineRemediationTransactions -eq 3) '10a must record three bounded remediation transactions'
    Assert-True ([int]$decisionJson.sideEffects.vultrUnattendedRestartEvents -eq 1) '10a must record one unattended restart event'
    Assert-True ([int]$decisionJson.sideEffects.vultrUnattendedRestartTransactions -eq 1) '10a must record one unattended restart transaction'
    Assert-True ([int]$decisionJson.sideEffects.vultrBoundedMutationEvents -eq 2) '10a must record two total bounded mutation events'
    Assert-True ([int]$decisionJson.sideEffects.vultrBoundedMutationTransactions -eq 4) '10a must record four total bounded mutation transactions'
    foreach ($name in @('rutgersRequests','productSourceMutations','dependencyMutations','databaseMutations','productBuilds','packageBuilds','releasePublications','productionMutations','gitMutations')) {
        Assert-True ([int]$decisionJson.sideEffects.$name -eq 0) "10a must record $name=0"
    }
    $decisionIds = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    $decisionStatusById = @{}
    foreach ($decision in @($decisionJson.decisions)) {
        Assert-True (-not [string]::IsNullOrWhiteSpace([string]$decision.id)) '10a contains an empty decision ID'
        Assert-True ($decisionIds.Add([string]$decision.id)) "10a contains duplicate decision ID $($decision.id)"
        Assert-True (-not [string]::IsNullOrWhiteSpace([string]$decision.status)) "10a decision $($decision.id) has no status"
        $decisionStatusById[[string]$decision.id] = [string]$decision.status
    }
    Assert-True ($decisionIds.Count -eq 22) "10a must contain 22 decisions, found $($decisionIds.Count)"
    Assert-True ([string]$decisionStatusById['P6-RD-011'] -eq 'APPROVED') '10a must record the five P7.5 tasks and total 32 as approved'
    Assert-True ([string]$decisionStatusById['P6-RD-016'] -eq 'APPROVED') '10a must record the live request budget as approved'
    Assert-True ([string]$decisionStatusById['P6-RD-018'] -eq 'COMPLETED_PASS') '10a must record bounded fwupd remediation as completed and passing'
    Assert-True ([string]$decisionStatusById['P6-RD-022'] -eq 'AUTHORIZED_AND_COMPLETED_PASS') '10a must record unattended-upgrades restart as authorized and completed'
}
Assert-TextMatch $decisionText '(?m)^p7_authorized=FALSE\s*$' '10 decision record must keep P7 unauthorized'
Assert-TextMatch $decisionText '(?m)^p7_plan_approved=TRUE\s*$' '10 decision record must approve the P7 plan'
Assert-TextMatch $decisionText '(?m)^live_request_budget_approved=TRUE\s*$' '10 decision record must approve the live request budget'
Assert-TextMatch $decisionText '(?m)^real_world_network_test_authorized=FALSE\s*$' '10 decision record must keep live network execution unauthorized'
Assert-TextMatch $decisionText '(?m)^vultr_baseline_remediation_authorized=TRUE\s*$' '10 decision record must authorize bounded fwupd remediation'
Assert-TextMatch $decisionText '(?m)^vultr_baseline_remediation_status=COMPLETED_PASS\s*$' '10 decision record must record fwupd remediation as completed and passing'
Assert-TextMatch $decisionText '(?m)^vultr_unattended_restart_authorized=TRUE\s*$' '10 decision record must authorize unattended-upgrades restart'
Assert-TextMatch $decisionText '(?m)^vultr_unattended_restart_status=COMPLETED_PASS\s*$' '10 decision record must record unattended-upgrades restart as completed and passing'
Assert-TextMatch $decisionText '(?m)^vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK\s*$' '10 decision record must keep P7.5 recheck on the healthy baseline'
Assert-TextMatch $decisionText '(?m)^vultr_staging_mutation_authorized=FALSE\s*$' '10 decision record must keep staging mutation unauthorized'
Assert-TextMatch $decisionText 'P6-RD-021[^\r\n]*NOT_AUTHORIZED' '10 decision record must explicitly record P7 start as not authorized'
Assert-TextMatch $decisionText 'P6-RD-022[^\r\n]*AUTHORIZED_AND_COMPLETED_PASS' '10 decision record must preserve the completed unattended-upgrades restart decision'

$gateJson = $null
try {
    $gateJson = (Get-Text $files.gateJson) | ConvertFrom-Json
} catch {
    Add-Failure "P6 review gate JSON is invalid: $($_.Exception.Message)"
}
if ($null -ne $gateJson) {
    Assert-True ([string]$gateJson.state -eq 'P6_REVIEW_READY') '09a state must be P6_REVIEW_READY'
    Assert-True ([string]$gateJson.p7State -eq 'NOT_STARTED_AWAITING_USER_APPROVAL') '09a must keep P7 not started and awaiting user approval'
    Assert-True ([string]$gateJson.productionDeploymentState -eq 'NOT_AUTHORIZED') '09a must keep production deployment unauthorized'
    Assert-True ([int]$gateJson.exactPackageCount -eq 2) '09a exactPackageCount must equal 2'
    Assert-True ([bool]$gateJson.p7PlanApproved) '09a must record the approved P7 plan'
    Assert-True ([bool]$gateJson.p75PlanApproved) '09a must record the approved P7.5 plan'
    Assert-True ([bool]$gateJson.liveRequestBudgetApproved) '09a must record the approved live request budget'
    Assert-RecursiveValue $gateJson @('p7Authorized') $false '09a must contain p7Authorized=false'
    Assert-RecursiveValue $gateJson @('realWorldNetworkTestAuthorized') $false '09a must contain realWorldNetworkTestAuthorized=false'
    Assert-True ([bool]$gateJson.vultrBaselineRemediationAuthorized) '09a must authorize the bounded Vultr baseline remediation'
    Assert-True ([string]$gateJson.vultrBaselineRemediationStatus -eq 'COMPLETED_PASS') '09a must record the completed baseline remediation'
    Assert-True ([bool]$gateJson.vultrUnattendedRestartAuthorized) '09a must record unattended-upgrades restart authorization'
    Assert-True ([string]$gateJson.vultrUnattendedRestartStatus -eq 'COMPLETED_PASS') '09a must record unattended-upgrades restart completion'
    Assert-True ([string]$gateJson.vultrStagingReadiness -eq 'BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK') '09a must record healthy baseline with P7.5 recheck'
    Assert-RecursiveValue $gateJson @('vultrStagingMutationAuthorized') $false '09a must contain vultrStagingMutationAuthorized=false'
    Assert-RecursiveValue $gateJson @('githubReleaseAuthorized') $false '09a must contain githubReleaseAuthorized=false'
    Assert-RecursiveValue $gateJson @('productionAuthorized', 'productionDeploymentAuthorized') $false '09a must contain productionAuthorized=false'
    foreach ($names in @(
        @('rutgersRequests', 'rutgers_requests'),
        @('productSourceMutations', 'product_source_mutations'),
        @('dependencyMutations', 'dependency_mutations'),
        @('databaseMutations', 'database_mutations'),
        @('productBuilds', 'product_builds'),
        @('packageBuilds', 'package_builds'),
        @('releasePublications', 'release_publications'),
        @('productionMutations', 'production_mutations'),
        @('gitMutations', 'git_mutations')
    )) {
        Assert-RecursiveValue $gateJson $names 0 "09a must record $($names[0])=0"
    }
    Assert-True ([int]$gateJson.sideEffects.vultrMutations -eq 2) '09a must record exactly two bounded Vultr mutation events'
    Assert-True ([int]$gateJson.sideEffects.vultrBaselineRemediationEvents -eq 1) '09a must record one bounded remediation event'
    Assert-True ([int]$gateJson.sideEffects.vultrBaselineRemediationTransactions -eq 3) '09a must record three bounded remediation transactions'
    Assert-True ([int]$gateJson.sideEffects.vultrUnattendedRestartEvents -eq 1) '09a must record one unattended restart event'
    Assert-True ([int]$gateJson.sideEffects.vultrUnattendedRestartTransactions -eq 1) '09a must record one unattended restart transaction'
    Assert-True ([int]$gateJson.sideEffects.vultrBoundedMutationEvents -eq 2) '09a must record two total bounded events'
    Assert-True ([int]$gateJson.sideEffects.vultrBoundedMutationTransactions -eq 4) '09a must record four total bounded transactions'
    Assert-RecursiveValue $gateJson @('p3Gate', 'p3InputGate') 'P3_PASS' '09a must record P3_PASS'
    Assert-RecursiveValue $gateJson @('p4Gate', 'p4InputGate') 'P4_PASS' '09a must record P4_PASS'
    Assert-RecursiveValue $gateJson @('p5Gate', 'p5InputGate') 'P5_PASS' '09a must record P5_PASS'
    Assert-RecursiveValue $gateJson @('p6Eligible', 'p6Eligibility') $true '09a must record P6 eligibility=true'
    Assert-RecursiveValue $gateJson @('taskRows', 'taskRowCount') $tasks.Count '09a task row count does not match 04'
    Assert-RecursiveValue $gateJson @('riskRows', 'riskRowCount') $risks.Count '09a risk row count does not match 06'
    Assert-RecursiveValue $gateJson @('traceRows', 'traceRowCount') $traceRows.Count '09a trace row count does not match 08'
    Assert-RecursiveValue $gateJson @('externalReadOnlyPreflightPerformed') $true '09a must record the completed external read-only preflight'
    Assert-RecursiveValue $gateJson @('windowsPrimarySqliteCount') 1 '09a must record one Windows primary SQLite database'
    Assert-RecursiveValue $gateJson @('windowsPrimarySqliteRelativePath') 'data/rbcsp.sqlite' '09a must record the Windows package-relative database path'
    Assert-RecursiveValue $gateJson @('linuxStateRoot') '/var/lib/bcsp' '09a must record the Linux external state root'
    Assert-RecursiveValue $gateJson @('containsDatabase') $false '09a must record that release packages contain no database'
    Assert-RecursiveValue $gateJson @('containsRealCatalogOpenData') $false '09a must record that release packages contain no real Catalog/Open data'
    Assert-RecursiveValue $gateJson @('p75RealWorldE2ERequired') $true '09a must require P7.5 real-world E2E'
    Assert-RecursiveValue $gateJson @('candidateHashesImmutableDuringP75') $true '09a must keep candidate hashes immutable in P7.5'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.initialSystemState -eq 'DEGRADED') '09a must retain the initial Vultr degraded state'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.initialFailedUnits -eq 2) '09a must retain the initial two failed units'
    Assert-RecursiveValue $gateJson @('blockingReasonCode') 'FWUPD_DAEMON_LIBRARY_VERSION_MISMATCH' '09a must retain the Vultr blocker reason code'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.remediationAuthorized) '09a must record the bounded baseline remediation authorization'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationExecutionStatus -eq 'COMPLETED_PASS') '09a must record baseline remediation as completed and passing'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.currentSystemState -eq 'RUNNING') '09a must record the current running state'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.currentFailedUnits -eq 0) '09a must record zero current failed units'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.exactInstallPackages).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.exactInstallPackages[0] -eq 'libfwupd3') '09a remediation install allowlist must contain only libfwupd3'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.exactUpgradePackages).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.exactUpgradePackages[0] -eq 'fwupd') '09a remediation upgrade allowlist must contain only fwupd'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.maximumRemovePackages -eq 0) '09a remediation must allow zero package removals'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.createSnapshot) '09a remediation must forbid snapshots'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.wholeMachineReboot) '09a remediation must forbid whole-machine reboot'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.installOrUpgradeOtherPackages) '09a remediation must forbid other package changes'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.authorizedRemediationScope.stopBeforeMutationIfDiffExpands) '09a remediation must stop before mutation if the diff expands'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.status -eq 'COMPLETED_PASS') '09a remediation result must pass'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.metadataRefresh -eq 'PASS') '09a metadata refresh must pass'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.remediationResult.postRefreshSimulationExactMatch) '09a post-refresh simulation must exactly match authorization'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.actualInstallPackages).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.remediationResult.actualInstallPackages[0] -eq 'libfwupd3') '09a actual install set must contain only libfwupd3'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.actualUpgradePackages).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.remediationResult.actualUpgradePackages[0] -eq 'fwupd') '09a actual upgrade set must contain only fwupd'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.actualRemovePackages).Count -eq 0) '09a actual remove set must be empty'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.fwupdServiceActiveState -eq 'active') '09a fwupd service must be active'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.fwupdServiceResult -eq 'success') '09a fwupd service result must be success'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.fwupdRefreshServiceResult -eq 'success') '09a fwupd-refresh result must be success'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.dpkgAuditLines -eq 0) '09a dpkg audit must be clean'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.heldPackages -eq 0) '09a must retain zero held packages'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.rebootRequired) '09a remediation must not require reboot'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.fwupdPendingChanges -eq 0) '09a fwupd must have zero pending changes'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.snapshotCreated) '09a remediation must not create snapshot'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.wholeMachineRebooted) '09a remediation must not reboot the machine'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.bcspChanged) '09a remediation must not change BCSP'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.caddyChanged) '09a remediation must not change Caddy'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.remediationEvents -eq 1) '09a must record one remediation event'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.transactions.total -eq 3) '09a must record three remediation transactions'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceCount -eq 1) '09a must record one residual needrestart service'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.services).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.services[0] -eq 'unattended-upgrades.service') '09a residual service must be unattended-upgrades.service only'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceActiveState -eq 'active') '09a residual service must be active'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceSubState -eq 'running') '09a residual service must be running'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceResult -eq 'success') '09a residual service result must be success'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.diagnosticMode -eq 'NEEDRESTART_VERBOSE_LIST') '09a must record verbose needrestart diagnostic mode'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.reasonCode -eq 'UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY') '09a must record the residual obsolete Python reason code'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.processRole -eq 'unattended-upgrade-shutdown') '09a must record the process role without a PID'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.obsoleteBinary -eq '/usr/bin/python3.12') '09a must record the obsolete Python binary path'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.suggestedAction -eq 'systemctl restart unattended-upgrades.service') '09a must retain the suggested service restart'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.needrestartKernelStatus -eq 1) '09a must retain needrestart kernel status 1'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.wholeMachineRebootRequired) '09a must record no whole-machine reboot requirement'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.authorizedPackageInstallSet).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.authorizedPackageInstallSet[0] -eq 'libfwupd3') '09a residual must preserve the exact authorized install set'
    Assert-True (@($gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.authorizedPackageUpgradeSet).Count -eq 1 -and [string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.authorizedPackageUpgradeSet[0] -eq 'fwupd') '09a residual must preserve the exact authorized upgrade set'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.pythonChangedByRemediation) '09a must record that remediation did not change Python'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.causalAttribution -eq 'NOT_ASSERTED') '09a must avoid unsupported residual causality'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceRestartAuthorized) '09a must authorize the residual service restart'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.serviceRestartPerformed) '09a must record the residual service restart'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.waiverGranted) '09a must record no residual waiver'
    Assert-True ([bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.residualCleared) '09a must record residual cleared'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.preRestartSafetyGate.status -eq 'PASS') '09a pre-restart safety gate must pass'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.preRestartSafetyGate.aptDailyState -eq 'inactive') '09a apt-daily must be inactive before restart'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.preRestartSafetyGate.aptDailyUpgradeState -eq 'inactive') '09a apt-daily-upgrade must be inactive before restart'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.preRestartSafetyGate.aptDpkgLocks -eq 'none') '09a must record no package locks before restart'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.preRestartSafetyGate.dpkgAuditLines -eq 0) '09a pre-restart dpkg audit must be clean'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.executedAction -eq 'systemctl restart unattended-upgrades.service') '09a must record the sole restart action'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.systemState -eq 'RUNNING') '09a post-restart systemd must be running'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.failedUnits -eq 0) '09a post-restart failed units must be zero'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.serviceActiveState -eq 'active') '09a unattended-upgrades must be active after restart'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.serviceSubState -eq 'running') '09a unattended-upgrades must be running after restart'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.serviceResult -eq 'success') '09a unattended-upgrades result must be success'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.fwupdServiceActiveState -eq 'active') '09a fwupd must remain active after restart'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.fwupdServiceResult -eq 'success') '09a fwupd result must remain success'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.fwupdRefreshServiceResult -eq 'success') '09a fwupd-refresh result must remain success'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.needrestartServiceCount -eq 0) '09a post-restart needrestart service count must be zero'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.dpkgAuditLines -eq 0) '09a post-restart dpkg audit must be clean'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.heldPackages -eq 0) '09a post-restart held packages must be zero'
    Assert-True (-not [bool]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.rebootRequired) '09a post-restart must not require reboot'
    Assert-True ([int]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.postRestart.fwupdPendingChanges -eq 0) '09a post-restart fwupd pending changes must be zero'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.stagingReadiness -eq 'BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK') '09a residual recovery must produce healthy baseline with recheck'
    Assert-True ([string]$gateJson.vultrReadOnlyPreflight.remediationResult.residualNeedrestart.mustRecheckAtTask -eq 'P7.5-001') '09a residual must be rechecked at P7.5-001'
    Assert-True (-not [bool]$gateJson.reviewConditions.newP75TasksRequireReview) '09a must record that P7.5 task review is complete'
    Assert-True ([bool]$gateJson.reviewConditions.newP75TasksApproved) '09a must record the approved P7.5 task set'
    Assert-True (-not [bool]$gateJson.reviewConditions.liveRequestBudgetRequiresReview) '09a must record that live budget review is complete'
    Assert-True ([bool]$gateJson.reviewConditions.liveRequestBudgetApproved) '09a must record the approved live budget'
    Assert-True ([bool]$gateJson.reviewConditions.vultrBaselineRemediationCompleted) '09a must record completed baseline remediation'
    Assert-True ([bool]$gateJson.reviewConditions.vultrStagingBaselineClean) '09a must record the current clean staging baseline'
    Assert-True ([bool]$gateJson.reviewConditions.unattendedUpgradesRestartAuthorized) '09a must record unattended-upgrades restart authorization'
    Assert-True ([bool]$gateJson.reviewConditions.unattendedUpgradesRestartCompleted) '09a must record unattended-upgrades restart completion'
    Assert-True (-not [bool]$gateJson.reviewConditions.unattendedUpgradesRestartWaived) '09a must not claim a waiver'
    Assert-True ([int]$gateJson.rowCounts.p7SubphaseRows.'P7.5' -eq 5) '09a must record five P7.5 tasks'
    Assert-True ([int]$gateJson.rowCounts.dependencyStatuses.APPROVED -eq 29) '09a must record 29 approved dependency rows'
    Assert-True ([int]$gateJson.rowCounts.dependencyStatuses.REJECTED -eq 13) '09a must record 13 rejected dependency rows'
}

Assert-TextMatch $gateText '(?m)^p6_gate=P6_REVIEW_READY\s*$' '09 gate must declare P6_REVIEW_READY'
Assert-TextMatch $gateText '(?m)^p7_plan_approved=TRUE\s*$' '09 gate must record the approved P7 plan'
Assert-TextMatch $gateText '(?m)^p7_5_plan_approved=TRUE\s*$' '09 gate must record the approved P7.5 plan'
Assert-TextMatch $gateText '(?m)^live_request_budget_approved=TRUE\s*$' '09 gate must record the approved live request budget'
Assert-TextMatch $gateText '(?m)^p7_status=NOT_STARTED_AWAITING_USER_APPROVAL\s*$' '09 gate must keep P7 not started'
Assert-TextMatch $gateText '(?m)^production_status=NOT_AUTHORIZED\s*$' '09 gate must keep production unauthorized'
Assert-TextMatch $gateText '(?m)^p7_authorized=FALSE\s*$' '09 gate must explicitly deny P7 authorization'
Assert-TextMatch $gateText '(?m)^real_world_network_test_authorized=FALSE\s*$' '09 gate must explicitly deny live network-test authorization'
Assert-TextMatch $gateText '(?m)^vultr_baseline_remediation_authorized=TRUE\s*$' '09 gate must authorize bounded fwupd remediation'
Assert-TextMatch $gateText '(?m)^vultr_baseline_remediation_status=COMPLETED_PASS\s*$' '09 gate must record fwupd remediation as completed and passing'
Assert-TextMatch $gateText '(?m)^vultr_initial_system_state=DEGRADED\s*$' '09 gate must preserve the initial degraded state'
Assert-TextMatch $gateText '(?m)^vultr_initial_failed_units=2\s*$' '09 gate must preserve the initial two failed units'
Assert-TextMatch $gateText '(?m)^vultr_current_system_state=RUNNING\s*$' '09 gate must record the current running state'
Assert-TextMatch $gateText '(?m)^vultr_current_failed_units=0\s*$' '09 gate must record zero current failed units'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_initial_service_count=1\s*$' '09 gate must preserve the initial residual count'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_service=unattended-upgrades\.service\s*$' '09 gate must name unattended-upgrades.service as the residual'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_kernel_status=1\s*$' '09 gate must retain needrestart kernel status 1'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_diagnostic_mode=NEEDRESTART_VERBOSE_LIST\s*$' '09 gate must retain verbose diagnostic mode'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_reason_code=UNATTENDED_UPGRADES_PROCESS_USES_OBSOLETE_PYTHON_BINARY\s*$' '09 gate must retain the obsolete Python reason code'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_obsolete_binary=/usr/bin/python3\.12\s*$' '09 gate must retain the obsolete binary path'
Assert-TextMatch $gateText '(?m)^vultr_residual_causal_attribution=NOT_ASSERTED\s*$' '09 gate must avoid unsupported residual causality'
Assert-TextMatch $gateText '(?m)^vultr_unattended_restart_safety_gate=PASS\s*$' '09 gate must record restart safety PASS'
Assert-TextMatch $gateText '(?m)^vultr_unattended_restart_authorized=TRUE\s*$' '09 gate must record restart authorization'
Assert-TextMatch $gateText '(?m)^vultr_unattended_restart_performed=TRUE\s*$' '09 gate must record restart execution'
Assert-TextMatch $gateText '(?m)^vultr_unattended_restart_waiver=FALSE\s*$' '09 gate must record no waiver'
Assert-TextMatch $gateText '(?m)^vultr_residual_needrestart_cleared=TRUE\s*$' '09 gate must record residual cleared'
Assert-TextMatch $gateText '(?m)^vultr_current_needrestart_service_count=0\s*$' '09 gate must record zero current needrestart services'
Assert-TextMatch $gateText '(?m)^vultr_python_changed_by_bounded_mutations=FALSE\s*$' '09 gate must record no Python changes'
Assert-TextMatch $gateText '(?m)^vultr_whole_machine_reboot_required=FALSE\s*$' '09 gate must record no whole-machine reboot requirement'
Assert-TextMatch $gateText '(?m)^vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK\s*$' '09 gate must record healthy baseline with P7.5 recheck'
Assert-TextMatch $gateText '(?m)^vultr_staging_mutation_authorized=FALSE\s*$' '09 gate must explicitly deny Vultr staging mutation authorization'
Assert-TextMatch $gateText '(?m)^github_release_authorized=FALSE\s*$' '09 gate must explicitly deny GitHub Release authorization'
Assert-TextMatch $gateText '(?m)^production_authorized=FALSE\s*$' '09 gate must explicitly deny production authorization'
foreach ($marker in @(
    'rutgers_requests',
    'product_source_mutations',
    'dependency_mutations',
    'database_mutations',
    'product_builds',
    'package_builds',
    'release_publications',
    'production_mutations',
    'git_mutations'
)) {
    Assert-TextMatch $gateText "(?m)^$marker=0\s*$" "09 gate must record $marker=0"
}
Assert-TextMatch $gateText '(?m)^vultr_remediation_events=2\s*$' '09 gate must record two bounded mutation events'
Assert-TextMatch $gateText '(?m)^vultr_remediation_transactions=4\s*$' '09 gate must record four bounded mutation transactions'
Assert-TextMatch $gateText '(?m)^vultr_mutations=2\s*$' '09 gate must record exactly two bounded Vultr mutation events'
Assert-TextMatch $gateText '(?m)^external_read_only_preflight_performed=TRUE\s*$' '09 gate must record the completed read-only Vultr preflight'
Assert-TextMatch $gateText '(?m)^task_count=32\s*$' '09 gate must record 32 total tasks'
Assert-TextMatch $gateText '(?m)^p7_5_task_count=5\s*$' '09 gate must record five P7.5 tasks'

$allP6Text = @($charterText, $localText, $publicText, $dagText, $verificationText, $authorizationText, $gateText, $decisionText) -join "`n"
Assert-True ($allP6Text -notmatch '(?im)^\s*p7_authorized\s*=\s*TRUE\s*$') 'an active P6 document prematurely authorizes P7'
Assert-True ($allP6Text -notmatch '(?im)^\s*(production|production_deployment)_authorized\s*=\s*TRUE\s*$') 'an active P6 document prematurely authorizes production deployment'
Assert-True ($allP6Text -notmatch '(?im)^\s*(real_world_network_test|vultr_staging_mutation|github_release)_authorized\s*=\s*TRUE\s*$') 'an active P6 document prematurely authorizes a post-Review action'
Assert-True ($allP6Text -notmatch '(?im)^\s*(rutgers_requests|product_source_mutations|dependency_mutations|database_mutations|product_builds|package_builds|release_publications|production_mutations|git_mutations)\s*=\s*[1-9][0-9]*\s*$') 'an active P6 document records a forbidden mutation or Rutgers request'
Assert-True ($allP6Text -notmatch '(?im)^\s*vultr_mutations\s*=\s*(?:0|1|[3-9]|[1-9][0-9]+)\s*$') 'an active P6 document must record exactly two bounded Vultr mutation events where the field is present'

if ($script:Failures.Count -gt 0) {
    Write-Output 'p6_gate=FAIL'
    Write-Output 'p7_status=NOT_STARTED'
    Write-Output 'production_status=NOT_AUTHORIZED'
    Write-Output "integrity_failures=$($script:Failures.Count)"
    foreach ($failure in $script:Failures) {
        Write-Output "failure=$failure"
    }
    exit 1
}

Write-Output 'p6_gate=P6_REVIEW_READY'
Write-Output 'p7_status=NOT_STARTED_AWAITING_USER_APPROVAL'
Write-Output 'production_status=NOT_AUTHORIZED'
Write-Output "task_rows=$($tasks.Count)"
Write-Output "p7_5_task_rows=$($p75.Count)"
Write-Output "dependency_approved=$($approvedRisks.Count)"
Write-Output "dependency_rejected=$($rejectedRisks.Count)"
Write-Output 'vultr_baseline_remediation=COMPLETED_PASS'
Write-Output 'vultr_current_system_state=RUNNING'
Write-Output 'vultr_current_failed_units=0'
Write-Output 'vultr_unattended_restart=COMPLETED_PASS'
Write-Output 'vultr_staging_readiness=BASELINE_HEALTHY_REQUIRES_P7_5_001_RECHECK'
Write-Output 'vultr_needrestart_service_count=0'
Write-Output 'integrity_failures=0'
