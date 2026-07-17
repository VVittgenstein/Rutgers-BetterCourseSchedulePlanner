param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path,
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\01-file-universe.tsv')
)

$ErrorActionPreference = 'Stop'

function Normalize-Path([string]$Path) {
    return $Path.Replace('\', '/')
}

function Get-Classification([string]$Path) {
    $result = [ordered]@{
        Capability = 'INTERNAL_ONLY'
        Disposition = 'OUT_OF_SCOPE'
        Ownership = 'HISTORICAL_EVIDENCE'
        RuntimeReachability = 'not-runtime'
        PackageReachability = 'must-not-package'
        Evidence = 'P2-PATH'
        NextAction = 'retain-as-evidence-only'
    }

    switch -Regex ($Path) {
        '^\.gitignore$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='INTERNAL_TOOLING';
            $result.PackageReachability='package-guard'; $result.NextAction='extend-positive-allowlist-and-secret-runtime-guards'; break
        }
        '^README\.md$' {
            $result.Capability='REQUIRED'; $result.Disposition='REWRITE'; $result.Ownership='LOCAL_ONLY';
            $result.PackageReachability='local-doc'; $result.NextAction='replace-node-mail-macos-claims-with-verified-local-quickstart'; break
        }
        '^Start-WebUI\.bat$' {
            $result.Capability='REQUIRED'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='LOCAL_ONLY';
            $result.RuntimeReachability='direct-user-entry'; $result.PackageReachability='local-package-required'; $result.NextAction='rewrite-wrapper-to-launch-bcsp-local-exe'; break
        }
        '^Start-WebUI\.command$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='legacy-direct-entry'; $result.NextAction='remove-from-current-product-and-local-package'; break
        }
        '^(package\.json|package-lock\.json|tsconfig\.json)$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='INTERNAL_TOOLING';
            $result.RuntimeReachability='legacy-node-toolchain'; $result.PackageReachability='must-not-copy-verbatim'; $result.NextAction='port-backend-consumers-to-rust-and-keep-only-proven-build-consumers'; break
        }
        '^api/src/routes/admin\.ts$|^api/tests/admin_mail_config\.test\.ts$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='registered-mail-runtime'; $result.NextAction='remove-mail-admin-route-tests-and-config-surface'; break
        }
        '^api/src/routes/sections\.ts$' {
            $result.Capability='REQUIRED'; $result.Disposition='REWRITE'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='registered-stub-runtime'; $result.NextAction='replace-stub-with-shared-section-search-and-detail-contract'; break
        }
        '^api/src/routes/subscriptions\.ts$|^api/src/routes/notifications\.local\.ts$|^api/tests/subscriptions\.test\.ts$|^api/tests/notifications\.local\.test\.ts$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='registered-legacy-watch-runtime'; $result.NextAction='extract-section-contract-and-rewrite-as-live-websocket-watch'; break
        }
        '^api/src/(queries/course_search|queries/filters|routes/courses|routes/filters|routes/health|routes/sharedSchemas|services/fetchRunner|config|container|plugins/requestLogging)\.ts$|^api/tests/(course_search|health)\.test\.ts$' {
            $result.Capability='REQUIRED'; $result.Disposition='PORT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='active-node-runtime'; $result.NextAction='port-with-recorded-defect-fixes-and-golden-tests'; break
        }
        '^api/src/routes/fetch\.ts$' {
            $result.Capability='REQUIRED'; $result.Disposition='PORT'; $result.Ownership='LOCAL_ONLY';
            $result.RuntimeReachability='registered-local-admin-runtime'; $result.NextAction='replace-destructive-manual-job-with-safe-catalog-refresh-control'; break
        }
        '^api/src/server\.ts$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='direct-server-entry'; $result.NextAction='port-lifecycle-errors-and-required-routes-remove-mail-add-static-ui-and-websocket'; break
        }
        '^api/src/types/fastify\.d\.ts$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REMOVE'; $result.Ownership='INTERNAL_TOOLING';
            $result.NextAction='remove-after-rust-backend-replaces-fastify-container-typing'; break
        }
        '^configs/mail_sender\.example\.json$|^configs/templates/email/' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='mail-config-or-asset'; $result.NextAction='remove-from-current-product-repository-surface-and-all-packages'; break
        }
        '^configs/fetch_pipeline\.(example\.json|schema\.json)$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='legacy-ingest-config'; $result.NextAction='port-only-proven-settings-remove-fake-switches-set-local-10m-default'; break
        }
        '^data/schema\.sql$|^data/migrations/001_init_schema\.sql$|^data/migrations/003_open_events\.sql$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='database-schema'; $result.NextAction='port-course-section-status-data-remove-persistent-personal-watch-and-mail-fanout'; break
        }
        '^data/migrations/(002_relax_section_index_scope|004_course_campus_locations)\.sql$' {
            $result.Capability='REQUIRED'; $result.Disposition='PORT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='database-migration'; $result.NextAction='port-as-migration-evidence-with-safe-composite-section-key'; break
        }
        '^frontend/(package\.json|package-lock\.json|i18n/messages\.json)$|^frontend/src/(App\.tsx|api/types\.ts|state/courseFilters\.ts)$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='frontend-runtime-or-build'; $result.PackageReachability='compiled-static-ui'; $result.NextAction='retain-required-ui-contract-remove-mail-future-and-dead-semantics'; break
        }
        '^frontend/src/(api/admin|components/MailSettingsPanel)\.(ts|tsx|css)$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='reachable-mail-ui'; $result.NextAction='remove-and-prove-zero-bundle-route-style-i18n-residue'; break
        }
        '^frontend/src/components/SchedulePreview\.(tsx|css)$' {
            $result.Capability='FUTURE'; $result.Disposition='DEFER'; $result.Ownership='FUTURE';
            $result.RuntimeReachability='orphan-future-component'; $result.NextAction='move-out-of-current-product-source-and-prove-artifact-absence'; break
        }
        '^frontend/src/dev/' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='SPLIT'; $result.Ownership='INTERNAL_TOOLING';
            $result.RuntimeReachability='orphan-dev-only'; $result.NextAction='extract-valid-fixtures-remove-future-calendar-fallback-from-current-build'; break
        }
        '^frontend/src/api/(subscriptions|notifications)\.ts$|^frontend/src/components/(SubscriptionCenter|SubscriptionManager|SubscribeButton|LocalSoundToggle)\.(tsx|css)$|^frontend/src/hooks/useLocalSoundNotifications\.ts$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='legacy-watch-ui-or-orphan'; $result.PackageReachability='compiled-static-ui'; $result.NextAction='extract-ui-audio-toast-value-rewrite-protocol-remove-email-and-persistent-active'; break
        }
        '^frontend/src/components/DataFetchCard\.(tsx|css)$|^frontend/src/api/fetchJobs\.ts$' {
            $result.Capability='REQUIRED'; $result.Disposition='REWRITE'; $result.Ownership='LOCAL_ONLY';
            $result.RuntimeReachability='reachable-local-data-admin-ui'; $result.PackageReachability='compiled-static-ui'; $result.NextAction='replace-crawler-log-surface-with-safe-freshness-refresh-settings'; break
        }
        '^frontend/(index\.html|README\.md)$|^frontend/src/' {
            $result.Capability='REQUIRED'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='frontend-runtime-or-build'; $result.PackageReachability='compiled-static-ui'; $result.NextAction='reuse-only-after-contract-accessibility-state-and-residue-fixes'; break
        }
        '^frontend/(tsconfig\.json|tsconfig\.node\.json|vite\.config\.ts)$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='INTERNAL_TOOLING';
            $result.PackageReachability='build-only'; $result.NextAction='retain-as-build-tooling-add-test-ws-and-clean-config-boundaries'; break
        }
        '^notifications/mail/' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='mail-runtime-or-test'; $result.NextAction='remove-complete-mail-code-test-dependency-asset-closure'; break
        }
        '^scripts/(mail_e2e_sim\.ts|mail_templates\.js)$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='mail-script'; $result.NextAction='remove-from-current-product-and-package'; break
        }
        '^scripts/oneclick_start\.js$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='LOCAL_ONLY';
            $result.RuntimeReachability='direct-local-launch-runtime'; $result.NextAction='extract-supervision-errors-remove-node-install-vite-dev-mail-and-rewrite-in-rust'; break
        }
        '^scripts/(run_stack|setup_local_env)\.sh$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='EXTRACT'; $result.Ownership='PUBLIC_DELTA';
            $result.RuntimeReachability='legacy-linux-dev-entry'; $result.NextAction='carry-ops-lessons-to-p4-not-local-package'; break
        }
        '^scripts/(soc_api_client|soc_normalizer|fetch_soc_data|migrate_db)\.(ts|js)$' {
            $result.Capability='REQUIRED'; $result.Disposition='PORT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='legacy-data-runtime'; $result.NextAction='port-algorithm-contract-with-recorded-correctness-and-safety-fixes'; break
        }
        '^scripts/backfill_core_attributes\.ts$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='MERGE'; $result.Ownership='INTERNAL_TOOLING';
            $result.NextAction='merge-compatible-core-normalization-into-ingest-migration'; break
        }
        '^scripts/i18n_missing_check\.ts$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='INTERNAL_TOOLING';
            $result.NextAction='retain-and-add-non-target-key-and-orphan-checks'; break
        }
        '^scripts/(soc_probe|soc_rate_limit|soc_field_matrix|incremental_trial|poller_resume_sim)\.' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='INTERNAL_TOOLING';
            $result.NextAction='retain-or-extract-as-isolated-validation-tool-never-user-package'; break
        }
        '^workers/mail_dispatcher\.ts$|^workers/tests/mail_dispatcher\.test\.ts$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.RuntimeReachability='mail-worker-or-test'; $result.NextAction='remove-mail-worker-and-test-closure'; break
        }
        '^workers/open_sections_poller\.ts$|^workers/tests/open_sections_poller\.auto\.test\.ts$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.RuntimeReachability='active-open-poller-or-test'; $result.NextAction='port-coalesced-polling-and-resilience-rewrite-watch-discovery-event-dedupe-fanout'; break
        }
        '^docs/(mail_sender_contract|mail_sender_usage|mail_worker_contract)\.md$' {
            $result.Capability='EXCLUDED'; $result.Disposition='REMOVE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.NextAction='remove-from-current-docs-and-all-packages'; break
        }
        '^docs/deployment_playbook\.md$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='PUBLIC_DELTA';
            $result.NextAction='carry-linux-production-lessons-to-p4-remove-from-local-user-package'; break
        }
        '^docs/(oneclick|quickstart)\.md$' {
            $result.Capability='REQUIRED'; $result.Disposition='REWRITE'; $result.Ownership='LOCAL_ONLY';
            $result.PackageReachability='local-doc'; $result.NextAction='rewrite-for-clean-windows-no-node-bcsp-local-exe'; break
        }
        '^docs/(subscription_model|notify_runbook|open_event_spec)\.md$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.NextAction='rewrite-live-watch-every-open-toast-max-and-sound-contract-remove-mail-persistence'; break
        }
        '^docs/ui_flow_course_list\.md$' {
            $result.Capability='MIXED_SEE_02'; $result.Disposition='SPLIT'; $result.Ownership='BASELINE_SHARED';
            $result.NextAction='rewrite-course-section-flow-extract-local-saved-views-remove-current-calendar-compact-share-claims'; break
        }
        '^docs/(query_api_contract|local_data_model|fetch_pipeline|data_load_runbook|data_refresh_strategy|i18n_key_map)\.md$' {
            $result.Capability='REQUIRED'; $result.Disposition='REWRITE'; $result.Ownership='BASELINE_SHARED';
            $result.NextAction='replace-drift-with-approved-contract-and-rust-data-lifecycle'; break
        }
        '^docs/(soc_api_notes|soc_field_matrix\.csv|soc_rate_limit.*)$' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='REUSE_WITH_FIXES'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.NextAction='retain-as-old-measurement-evidence-revalidate-before-runtime-policy'; break
        }
        '^reports/mail_worker_latency\.md$' {
            $result.Capability='EXCLUDED'; $result.Disposition='OUT_OF_SCOPE'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.NextAction='retain-only-as-old-failure-evidence-never-package'; break
        }
        '^reports/' {
            $result.Capability='INTERNAL_ONLY'; $result.Disposition='EXTRACT'; $result.Ownership='HISTORICAL_EVIDENCE';
            $result.NextAction='extract-stable-fixtures-and-methods-retain-report-as-history-never-package'; break
        }
    }

    return [pscustomobject]$result
}

Push-Location $RepositoryRoot
try {
    $tracked = @(git ls-files | ForEach-Object { Normalize-Path $_ })
    $rootFiles = @('.gitignore','README.md','Start-WebUI.bat','Start-WebUI.command','package.json','package-lock.json','tsconfig.json')

    $files = @($tracked | Where-Object {
        $_ -in $rootFiles -or
        $_ -like 'api/*' -or
        $_ -like 'configs/*' -or
        $_ -eq 'data/schema.sql' -or
        $_ -like 'data/migrations/*' -or
        $_ -like 'frontend/*' -or
        $_ -like 'notifications/*' -or
        $_ -like 'scripts/*' -or
        $_ -like 'workers/*' -or
        ($_ -like 'docs/*' -and $_ -notlike 'docs/archive/*' -and $_ -notlike 'docs/p1-a-recovery/*' -and $_ -ne 'docs/deliverable-a-windows-local-release-requirements.md') -or
        $_ -like 'reports/*'
    } | Sort-Object -Unique)

    if ($files.Count -ne 158) {
        throw "Expected 158 owned product files, found $($files.Count). Refuse to emit a silently drifted baseline."
    }

    $header = @(
        'row_id','path','bytes','lines','sha256','git_state','worktree_state','type',
        'runtime_package_reachability','capability','disposition','ownership','evidence_id','next_action'
    ) -join "`t"
    $rows = [System.Collections.Generic.List[string]]::new()
    $rows.Add($header)

    $index = 0
    foreach ($path in $files) {
        $index++
        $item = Get-Item -LiteralPath $path
        $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
        $lineCount = if ($item.Length -eq 0) { 0 } else { @(Get-Content -LiteralPath $path -Encoding utf8).Count }
        $dirtyRaw = @(git status --short -- $path)
        $dirty = if ($dirtyRaw.Count) { ($dirtyRaw -join ',').Trim() } else { 'clean' }
        $extension = if ($item.Extension) { $item.Extension.TrimStart('.').ToLowerInvariant() } else { 'none' }
        $class = Get-Classification $path
        $reach = "$($class.RuntimeReachability)|$($class.PackageReachability)"
        $values = @(
            ('F{0:D3}' -f $index), $path, $item.Length, $lineCount, $hash, 'tracked', $dirty, $extension,
            $reach, $class.Capability, $class.Disposition, $class.Ownership, $class.Evidence, $class.NextAction
        ) | ForEach-Object { ([string]$_).Replace("`t", ' ').Replace("`r", ' ').Replace("`n", ' ') }
        $rows.Add(($values -join "`t"))
    }

    $rows | Set-Content -LiteralPath $OutputPath -Encoding utf8
    $outputHash = (Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256).Hash
    Write-Output "owned_product_count=$($files.Count)"
    Write-Output "output=$OutputPath"
    Write-Output "sha256=$outputHash"
}
finally {
    Pop-Location
}
