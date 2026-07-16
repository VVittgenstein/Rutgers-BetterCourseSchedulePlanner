import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import { ShellStyles } from './application';
import {
  ActionButton,
  DesignSystemStyles,
  StatePanel,
} from './design-system';
import type { SupportedLocale } from './i18n/contract';
import { useBcspI18n, type BcspI18nRuntime } from './i18n/runtime';
import {
  PRODUCT_PROTOCOL_VERSION,
  isServiceStatusV2,
  useProductRuntimeState,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionKey,
  type ServiceOperationPhaseV1,
  type ServiceStatus,
  type WatchPolicyV1,
  searchDataProgress,
} from './product';
import {
  SearchSessionProvider,
  SearchWorkspace,
  useSearchSession,
  type QueryScopeUnavailableActionRenderer,
} from './search';
import { AppRouterProvider, RouterLink, useAppRouter } from './routing';
import {
  useServiceStatus,
  useShellDataState,
  type ServiceStatusResource,
  type ShellDataState,
} from './shell';
import {
  LiveWatchProvider,
  useLiveWatch,
  WatchToastRegion,
  WatchWorkspace,
  WatchWorkspaceStyles,
} from './watch';

function retryBootstrap() {
  globalThis.location?.reload();
}

interface ServiceStatusPublisherValue {
  readonly publish: (resource: ServiceStatusResource | null) => void;
}

const ServiceStatusPublisherContext = createContext<ServiceStatusPublisherValue | null>(null);

function ServiceStatusPublisher({ resource }: { readonly resource: ServiceStatusResource }) {
  const publisher = useContext(ServiceStatusPublisherContext);
  useEffect(() => {
    publisher?.publish(resource);
    return () => publisher?.publish(null);
  }, [publisher, resource]);
  return null;
}

const BOOTSTRAP_SERVICE_RESOURCE: ServiceStatusResource = {
  connection: 'CONNECTING',
  retry: retryBootstrap,
  revision: 'bootstrap',
  snapshot: null,
};

const BOOTSTRAP_ERROR_RESOURCE: ServiceStatusResource = {
  ...BOOTSTRAP_SERVICE_RESOURCE,
  connection: 'INTERRUPTED',
};

export interface SharedWorkspaceExtension {
  readonly content: ReactNode;
  readonly intro: string;
  readonly navigationLabel: string;
  readonly path: string;
  readonly sequence: string;
  readonly title: string;
}

export interface SharedExperienceConfiguration {
  readonly initialFilters?: FilterStateV1 | undefined;
  readonly initialSelectedSections?: readonly SectionKey[] | undefined;
  readonly initialVolume?: number | undefined;
  readonly initialWatchPolicy?: WatchPolicyV1 | undefined;
  readonly onFiltersChange?: ((filters: FilterStateV1) => void) | undefined;
  readonly onSelectedSectionsChange?: ((selected: readonly SectionKey[]) => void) | undefined;
  readonly onVolumeChange?: ((volume: number) => void) | undefined;
  readonly onWatchPolicyChange?: ((policy: WatchPolicyV1) => void) | undefined;
  readonly renderUnavailableScopeAction?: QueryScopeUnavailableActionRenderer | undefined;
}

export interface SharedApplicationProps {
  readonly experience?: SharedExperienceConfiguration | undefined;
  readonly onLocaleChange?: ((locale: SupportedLocale) => void) | undefined;
  readonly workspaceExtensions?: readonly SharedWorkspaceExtension[] | undefined;
}

function activeExtension(
  pathname: string,
  extensions: readonly SharedWorkspaceExtension[],
): SharedWorkspaceExtension | undefined {
  return extensions.find((extension) => extension.path === pathname);
}

function LanguageControl({
  i18n,
  onLocaleChange,
}: {
  readonly i18n: BcspI18nRuntime;
  readonly onLocaleChange?: ((locale: SupportedLocale) => void) | undefined;
}) {
  const changeLocale = (locale: SupportedLocale) => {
    void i18n.changeLocale(locale).then(() => onLocaleChange?.(locale));
  };
  return (
    <fieldset className="bcsp-language">
      <legend className="bcsp-visually-hidden">{i18n.t('app.locale_control')}</legend>
      <button
        aria-pressed={i18n.locale === 'en-US'}
        className="bcsp-language__button"
        onClick={() => changeLocale('en-US')}
        type="button"
      >
        EN / US
      </button>
      <button
        aria-pressed={i18n.locale === 'zh-CN'}
        className="bcsp-language__button"
        onClick={() => changeLocale('zh-CN')}
        type="button"
      >
        中文 / CN
      </button>
    </fieldset>
  );
}

function ShellFrame({
  children,
  i18n,
  onLocaleChange,
  workspaceExtensions,
}: {
  readonly children: ReactNode;
  readonly i18n: BcspI18nRuntime;
  readonly onLocaleChange?: ((locale: SupportedLocale) => void) | undefined;
  readonly workspaceExtensions: readonly SharedWorkspaceExtension[];
}) {
  const { navigate, pathname } = useAppRouter();
  const navigationRef = useRef<HTMLElement | null>(null);
  const [statusResource, setStatusResource] = useState<ServiceStatusResource | null>(null);
  const publishStatus = useCallback((resource: ServiceStatusResource | null) => {
    setStatusResource(resource);
  }, []);
  const statusPublisher = useMemo(() => ({ publish: publishStatus }), [publishStatus]);
  const extension = activeExtension(pathname, workspaceExtensions);
  const sectionWorkspace = pathname.startsWith('/sections');
  const watchWorkspace = pathname === '/watch';
  const directSection = sectionWorkspace && pathname !== '/sections';
  const courseWorkspace = sectionWorkspace || (!watchWorkspace && extension === undefined);
  const extensionIndex = extension === undefined ? -1 : workspaceExtensions.indexOf(extension);
  const sequence = extension === undefined
    ? (watchWorkspace ? '02' : '01')
    : String(extensionIndex + 3).padStart(2, '0');
  const workspaceTitle = extension?.title ?? (watchWorkspace
    ? i18n.t('app.nav_watch')
    : directSection
    ? i18n.t('search.section_detail_title')
    : i18n.t('search.course_workspace'));
  const workspaceIntro = extension?.intro ?? (watchWorkspace
    ? i18n.t('watch.desk_lede')
    : i18n.t('search.course_intro'));
  const workspaceLabel = extension?.navigationLabel
    ?? (watchWorkspace ? i18n.t('app.nav_watch') : i18n.t('app.nav_courses'));

  useEffect(() => {
    if (pathname === '/sections') navigate('/', { replace: true });
  }, [navigate, pathname]);

  useEffect(() => {
    const navigation = navigationRef.current;
    const root = globalThis.document?.documentElement;
    if (navigation === null || root === undefined) return undefined;
    const measure = () => {
      root.style.setProperty('--bcsp-navigation-height', `${navigation.getBoundingClientRect().height}px`);
    };
    measure();
    globalThis.addEventListener('resize', measure);
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(navigation);
    return () => {
      observer?.disconnect();
      globalThis.removeEventListener('resize', measure);
      root.style.removeProperty('--bcsp-navigation-height');
    };
  }, [workspaceExtensions.length]);

  return (
    <ServiceStatusPublisherContext.Provider value={statusPublisher}>
      <DesignSystemStyles />
      <ShellStyles />
      <a className="bcsp-skip-link" href="#bcsp-workspace">
        {i18n.t('app.skip_to_workspace')}
      </a>
      <header className="bcsp-masthead">
        <div className="bcsp-masthead__identity">
          <p className="bcsp-masthead__eyebrow">[ {i18n.t('app.console')} ]</p>
          <h1 className="bcsp-masthead__title">
            <span className="bcsp-masthead__mark">{i18n.t('app.brand_short')}</span>
            <span className="bcsp-masthead__name">{i18n.t('app.title')}</span>
          </h1>
        </div>
        <div className="bcsp-masthead__utility">
          <div className="bcsp-utility-copy">
            <span>{i18n.t('app.interface_revision')}</span>
            <strong>RU / SOC</strong>
          </div>
          <LanguageControl i18n={i18n} onLocaleChange={onLocaleChange} />
        </div>
      </header>
      <nav
        aria-label={i18n.t('app.navigation')}
        className="bcsp-navigation"
        data-extended={workspaceExtensions.length > 0 || undefined}
        ref={navigationRef}
      >
        <RouterLink
          aria-current={courseWorkspace ? 'page' : undefined}
          className="bcsp-navigation__link"
          data-active={courseWorkspace || undefined}
          to="/"
        >
          <span>01</span>{i18n.t('app.nav_courses')}
        </RouterLink>
        <RouterLink
          aria-current={watchWorkspace ? 'page' : undefined}
          className="bcsp-navigation__link"
          data-active={watchWorkspace || undefined}
          to="/watch"
        >
          <span>02</span>{i18n.t('app.nav_watch')}
        </RouterLink>
        {workspaceExtensions.map((workspace, index) => (
          <RouterLink
            aria-current={extension?.path === workspace.path ? 'page' : undefined}
            className="bcsp-navigation__link"
            data-active={extension?.path === workspace.path || undefined}
            key={workspace.path}
            to={workspace.path}
          >
            <span>{String(index + 3).padStart(2, '0')}</span>{workspace.navigationLabel}
          </RouterLink>
        ))}
      </nav>
      <main className="bcsp-main" id="bcsp-workspace" tabIndex={-1}>
        <section className="bcsp-workspace" aria-labelledby="bcsp-workspace-title">
          <header className="bcsp-workspace__heading">
            <div className="bcsp-workspace__identity">
              <p className="bcsp-section-label">[ {sequence} / {workspaceLabel} ]</p>
              <div className="bcsp-workspace__title-line">
                <span aria-hidden="true" className="bcsp-workspace__sequence">{sequence}</span>
                <h2 className="bcsp-workspace__title" id="bcsp-workspace-title">
                  {workspaceTitle}
                </h2>
              </div>
              <p className="bcsp-workspace__intro">{workspaceIntro}</p>
            </div>
            <div className="bcsp-workspace__status-slot">
              {statusResource === null ? null : <ServiceStatusBand resource={statusResource} />}
              <p className="bcsp-workspace__protocol">
                {i18n.t('app.protocol')} / BCSP.V{PRODUCT_PROTOCOL_VERSION}
              </p>
            </div>
          </header>
          {children}
        </section>
      </main>
      <footer className="bcsp-footer">
        <span className="bcsp-footer__copyright">
          Copyright (c) 2026 VVittgenstein
        </span>
        <span className="bcsp-footer__protocol">
          {i18n.t('app.protocol')} / BCSP.V{PRODUCT_PROTOCOL_VERSION}
        </span>
      </footer>
    </ServiceStatusPublisherContext.Provider>
  );
}

function InitialState({
  detail,
  heading,
  kind,
  retry,
}: {
  readonly detail: string;
  readonly heading: string;
  readonly kind: 'loading' | 'empty' | 'error';
  readonly retry?: (() => void) | undefined;
}) {
  const { t } = useBcspI18n();
  return (
    <div className="bcsp-state-wrap">
      <StatePanel
        action={retry === undefined ? undefined : (
          <ActionButton onClick={retry} tone="accent">{t('action.retry')}</ActionButton>
        )}
        detail={detail}
        heading={heading}
        kind={kind}
      />
    </div>
  );
}

const SERVICE_PHASE_KEYS = {
  STARTING: 'service.phase.starting',
  DISCOVERING: 'service.phase.discovering',
  CATALOG_FETCH: 'service.phase.catalog_fetch',
  CATALOG_PROCESS: 'service.phase.catalog_process',
  CATALOG_PUBLISH: 'service.phase.catalog_publish',
  OPEN_FETCH: 'service.phase.open_fetch',
  IDLE: 'service.phase.idle',
  RETRY_WAIT: 'service.phase.retry_wait',
  STOPPED: 'service.phase.stopped',
} as const satisfies Record<ServiceOperationPhaseV1, Parameters<BcspI18nRuntime['t']>[0]>;

const SERVICE_STAGE_KEYS = {
  CATALOG_FETCH: 'service.phase.catalog_fetch',
  CATALOG_PROCESS: 'service.phase.catalog_process',
  OPEN_FETCH: 'service.phase.open_fetch',
  SNAPSHOT_PUBLISH: 'service.phase.catalog_publish',
} as const;

function targetLabel(status: ServiceStatus): string | null {
  if (isServiceStatusV2(status)) {
    const target = status.operations[0]?.target;
    return target === undefined ? null : `${target.term} / ${target.campus}`;
  }
  const target = status.operation.target;
  return target === null ? null : `${target.term} / ${target.campus}`;
}

function ServiceStatusBand({ resource }: { readonly resource: ServiceStatusResource }) {
  const i18n = useBcspI18n();
  const status = resource.snapshot;
  const interrupted = resource.connection === 'INTERRUPTED';
  const progress = searchDataProgress(status);
  const retrying = status !== null && (isServiceStatusV2(status)
    ? status.targets.some(({ nextRetryAt, workState }) =>
      workState === 'RETRY_WAIT' && nextRetryAt !== null)
    : status.operation.phase === 'RETRY_WAIT');
  const incompleteWithIssue = !progress.ready && (status?.issues.length ?? 0) > 0;
  const failed = interrupted || status?.level === 'ERROR' || retrying || incompleteWithIssue;
  const level = interrupted
    ? i18n.t('service.connection_interrupted')
    : progress.ready && retrying
      ? i18n.t('service.overall_ready_retrying')
      : progress.ready
        ? i18n.t('service.overall_ready')
        : failed
      ? i18n.t('service.overall_retrying')
      : progress.anyReady
        ? i18n.t('service.overall_partial', { count: progress.current })
      : i18n.t('service.overall_preparing');
  const phase = status === null
    ? i18n.t('service.phase.starting')
    : isServiceStatusV2(status)
      ? status.operations[0] === undefined
        ? i18n.t('service.phase.idle')
        : i18n.t(SERVICE_STAGE_KEYS[status.operations[0].stage])
      : i18n.t(SERVICE_PHASE_KEYS[status.operation.phase]);
  const target = status === null ? null : targetLabel(status);
  const summaries = status !== null && isServiceStatusV2(status)
    ? status.automaticTermSummaries
    : null;
  return (
    <section
      aria-label={i18n.t('app.system_status')}
      className="bcsp-service-status"
      data-connection={resource.connection.toLowerCase()}
      data-expanded={interrupted || retrying || !progress.ready || undefined}
      data-level={status?.level.toLowerCase() ?? 'initializing'}
      id="bcsp-system-status"
    >
      <div className="bcsp-service-status__lead">
        <span
          className="bcsp-service-status__signal"
          data-loading={!progress.ready && !failed || undefined}
          aria-hidden="true"
        />
        <div>
          <p className="bcsp-service-status__kicker">{i18n.t('service.status_label')}</p>
          <p className="bcsp-service-status__headline">{level}</p>
        </div>
      </div>
      <div className="bcsp-service-status__operation">
        <span className="bcsp-service-status__label">{i18n.t('service.current_operation')}</span>
        <strong>{phase}</strong>
      </div>
      <div className="bcsp-service-status__progress">
        <progress
          aria-label={i18n.t('service.overall_progress')}
          max={Math.max(1, progress.total)}
          value={progress.current}
        />
        <span>{i18n.t('service.progress_percent', { percent: progress.percent })}</span>
      </div>
      <dl className="bcsp-service-status__counts">
        {summaries === null ? (
          <>
            <div>
              <dt>{i18n.t('service.catalog')}</dt>
              <dd>{`${progress.catalog.current}/${progress.catalog.total}`}</dd>
            </div>
            <div>
              <dt>{i18n.t('service.open')}</dt>
              <dd>{`${progress.open.current}/${progress.open.total}`}</dd>
            </div>
          </>
        ) : summaries.map((summary) => (
          <div key={summary.term}>
            <dt><samp>{summary.term}</samp></dt>
            <dd>{`${summary.readyTargetCount}/${summary.totalTargetCount}`}</dd>
          </div>
        ))}
      </dl>
      <details className="bcsp-service-status__detail">
        <summary>{i18n.t('service.diagnostics')}</summary>
        <div className="bcsp-service-status__diagnostics">
          <p>{interrupted ? i18n.t('service.connection_interrupted') : i18n.t('service.activity_detail')}</p>
          {target === null ? null : <samp>{target}</samp>}
          {status !== null && isServiceStatusV2(status) ? status.operations.map((operation, index) => (
            <samp key={`${operation.target.term}:${operation.target.campus}:${operation.stage}:${operation.startedAt}:${index}`}>
              {operation.target.term} / {operation.target.campus} / {operation.stage}
            </samp>
          )) : null}
          {status !== null && isServiceStatusV2(status) ? status.targets
            .filter(({ error }) => error !== null)
            .map(({ error, stage, target: errorTarget }) => (
              <samp key={`${errorTarget.term}:${errorTarget.campus}:${error?.traceId ?? ''}`}>
                {errorTarget.term} / {errorTarget.campus} / {stage ?? 'IDLE'} / {error?.code}
              </samp>
            )) : null}
          {status?.issues.map((issue) => (
            <samp key={`${issue.component}:${issue.code}:${issue.target?.term ?? ''}:${issue.target?.campus ?? ''}`}>
              {issue.component} / {issue.code}
            </samp>
          ))}
          {interrupted ? (
            <button className="bcsp-service-status__retry" onClick={resource.retry} type="button">
              {i18n.t('action.retry')}
            </button>
          ) : null}
        </div>
      </details>
      <p className="bcsp-visually-hidden" aria-atomic="true" aria-live="polite" role="status">
        {interrupted ? i18n.t('service.connection_interrupted') : `${level}. ${phase}`}
      </p>
    </section>
  );
}

function ReadyCatalogContent({
  experience,
  runtime,
  serviceStatus,
  state,
}: {
  readonly experience: SharedExperienceConfiguration;
  readonly runtime: ProductRuntimePort;
  readonly serviceStatus: ServiceStatus | null;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  const { pathname } = useAppRouter();

  return (
    <>
      <div hidden={pathname === '/watch'}>
          <SearchWorkspace
            initialFilters={experience.initialFilters}
            onFiltersChange={experience.onFiltersChange}
            renderUnavailableScopeAction={experience.renderUnavailableScopeAction}
            runtime={runtime}
            serviceStatus={serviceStatus}
            shellState={state}
          />
      </div>
      <div hidden={pathname !== '/watch'}>
        <WatchWorkspace
          initialPolicy={experience.initialWatchPolicy}
          onPolicyChange={experience.onWatchPolicyChange}
        />
      </div>
    </>
  );
}

function ReadyCatalog({
  experience,
  runtime,
  serviceStatus,
  state,
}: {
  readonly runtime: ProductRuntimePort;
  readonly serviceStatus: ServiceStatus | null;
  readonly experience: SharedExperienceConfiguration;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  return (
    <ReadyCatalogContent
      experience={experience}
      runtime={runtime}
      serviceStatus={serviceStatus}
      state={state}
    />
  );
}

function ReadyRuntime({
  experience,
  runtime,
}: {
  readonly experience: SharedExperienceConfiguration;
  readonly runtime: ProductRuntimePort;
}) {
  const { t } = useBcspI18n();
  const searchSession = useSearchSession();
  const { updateWatchableTerms } = useLiveWatch();
  const service = useServiceStatus(
    runtime,
    undefined,
    searchSession.state.appliedScope ?? undefined,
  );
  const watchableTermKey = service.snapshot !== null && isServiceStatusV2(service.snapshot)
    ? `${service.snapshot.termWindow.currentTerm}\u0000${service.snapshot.termWindow.nextTerm}`
    : '';
  useEffect(() => {
    if (service.snapshot !== null && isServiceStatusV2(service.snapshot)) {
      updateWatchableTerms([
        service.snapshot.termWindow.currentTerm,
        service.snapshot.termWindow.nextTerm,
      ]);
    } else {
      updateWatchableTerms([]);
    }
  }, [service.snapshot, updateWatchableTerms, watchableTermKey]);
  const { retry, state } = useShellDataState(runtime, service.revision);
  if (state.status === 'LOADING') {
    return <><ServiceStatusPublisher resource={service} /><InitialState detail={t('app.loading_body')} heading={t('app.loading_title')} kind="loading" /></>;
  }
  if (state.status === 'ERROR') {
    return (
      <><ServiceStatusPublisher resource={service} /><InitialState
          detail={t('app.catalog_error_body')}
          heading={t('app.catalog_error_title')}
          kind="error"
          retry={retry}
        /></>
    );
  }
  if (state.status === 'EMPTY') {
    return (
      <><ServiceStatusPublisher resource={service} /><InitialState
          detail={t('app.no_targets_body')}
          heading={t('app.no_targets_title')}
          kind="empty"
          retry={retry}
        /></>
    );
  }
  return <><ServiceStatusPublisher resource={service} /><ReadyCatalog
      experience={experience}
      runtime={runtime}
      serviceStatus={service.snapshot}
      state={state}
    /></>;
}

function ReadyProduct({
  experience,
  runtime,
  workspaceExtensions,
}: {
  readonly experience: SharedExperienceConfiguration;
  readonly runtime: ProductRuntimePort;
  readonly workspaceExtensions: readonly SharedWorkspaceExtension[];
}) {
  const { pathname } = useAppRouter();
  const extension = activeExtension(pathname, workspaceExtensions);
  return (
    <SearchSessionProvider>
      <LiveWatchProvider
        initialSelected={experience.initialSelectedSections}
        initialWatchableTerms={[]}
        initialVolume={experience.initialVolume}
        onSelectedChange={experience.onSelectedSectionsChange}
        onVolumeChange={experience.onVolumeChange}
        runtime={runtime}
      >
        <WatchWorkspaceStyles />
        <div hidden={extension !== undefined}>
          <ReadyRuntime experience={experience} runtime={runtime} />
        </div>
        {extension?.content ?? null}
        <WatchToastRegion />
      </LiveWatchProvider>
    </SearchSessionProvider>
  );
}

export function SharedApplication({
  experience = {},
  onLocaleChange,
  workspaceExtensions = [],
}: SharedApplicationProps = {}) {
  const i18n = useBcspI18n();
  const productRuntime = useProductRuntimeState();
  return (
    <AppRouterProvider>
      <div
        className="bcsp-shell"
        data-bcsp-locale={i18n.locale}
        data-bcsp-product-error={
          productRuntime.status === 'ERROR' ? productRuntime.reason : undefined
        }
        data-bcsp-product-protocol={PRODUCT_PROTOCOL_VERSION}
        data-bcsp-product-state={productRuntime.status}
        data-bcsp-shared-application=""
      >
        <ShellFrame
          i18n={i18n}
          onLocaleChange={onLocaleChange}
          workspaceExtensions={workspaceExtensions}
        >
          {productRuntime.status === 'LOADING' ? (
            <>
              <ServiceStatusPublisher resource={BOOTSTRAP_SERVICE_RESOURCE} />
              <InitialState
                detail={i18n.t('app.loading_body')}
                heading={i18n.t('app.loading_title')}
                kind="loading"
              />
            </>
          ) : productRuntime.status === 'ERROR' ? (
            <>
              <ServiceStatusPublisher resource={BOOTSTRAP_ERROR_RESOURCE} />
              <InitialState
                detail={i18n.t('app.bootstrap_error_body')}
                heading={i18n.t('app.bootstrap_error_title')}
                kind="error"
                retry={retryBootstrap}
              />
            </>
          ) : (
            <ReadyProduct
              experience={experience}
              runtime={productRuntime.runtime}
              workspaceExtensions={workspaceExtensions}
            />
          )}
        </ShellFrame>
      </div>
    </AppRouterProvider>
  );
}
