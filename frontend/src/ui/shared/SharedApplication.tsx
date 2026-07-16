import type { ReactNode } from 'react';

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
  useProductRuntimeState,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionKey,
  type ServiceLevelV1,
  type ServiceOperationPhaseV1,
  type ServiceStatusV1,
  type WatchPolicyV1,
} from './product';
import { SearchWorkspace } from './search';
import { AppRouterProvider, RouterLink, useAppRouter } from './routing';
import {
  useServiceStatus,
  useShellDataState,
  type ServiceStatusResource,
  type ShellDataState,
} from './shell';
import {
  LiveWatchProvider,
  WatchToastRegion,
  WatchWorkspace,
  WatchWorkspaceStyles,
} from './watch';

function retryBootstrap() {
  globalThis.location?.reload();
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
  const { pathname } = useAppRouter();
  const extension = activeExtension(pathname, workspaceExtensions);
  const sectionWorkspace = pathname.startsWith('/sections');
  const watchWorkspace = pathname === '/watch';
  const directSection = sectionWorkspace && pathname !== '/sections';
  const courseWorkspace = !sectionWorkspace && !watchWorkspace && extension === undefined;
  const sequence = extension?.sequence ?? (watchWorkspace ? '03' : sectionWorkspace ? '02' : '01');
  const workspaceTitle = extension?.title ?? (watchWorkspace
    ? i18n.t('app.nav_watch')
    : directSection
    ? i18n.t('search.section_detail_title')
    : sectionWorkspace
      ? i18n.t('search.section_workspace')
      : i18n.t('search.course_workspace'));
  const workspaceIntro = extension?.intro ?? (watchWorkspace
    ? i18n.t('watch.desk_lede')
    : sectionWorkspace
    ? i18n.t('search.section_intro')
    : i18n.t('search.course_intro'));
  return (
    <>
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
      >
        <span className="bcsp-navigation__label">[ {i18n.t('app.catalog_workspace')} ]</span>
        <RouterLink
          aria-current={courseWorkspace ? 'page' : undefined}
          className="bcsp-navigation__link"
          data-active={courseWorkspace || undefined}
          to="/"
        >
          <span>01</span>{i18n.t('app.nav_courses')}
        </RouterLink>
        <RouterLink
          aria-current={sectionWorkspace ? 'page' : undefined}
          className="bcsp-navigation__link"
          data-active={sectionWorkspace || undefined}
          to="/sections"
        >
          <span>02</span>{i18n.t('app.nav_sections')}
        </RouterLink>
        <RouterLink
          aria-current={watchWorkspace ? 'page' : undefined}
          className="bcsp-navigation__link"
          data-active={watchWorkspace || undefined}
          to="/watch"
        >
          <span>03</span>{i18n.t('app.nav_watch')}
        </RouterLink>
        {workspaceExtensions.map((workspace) => (
          <RouterLink
            aria-current={extension?.path === workspace.path ? 'page' : undefined}
            className="bcsp-navigation__link"
            data-active={extension?.path === workspace.path || undefined}
            key={workspace.path}
            to={workspace.path}
          >
            <span>{workspace.sequence}</span>{workspace.navigationLabel}
          </RouterLink>
        ))}
      </nav>
      <main className="bcsp-main" id="bcsp-workspace" tabIndex={-1}>
        <section className="bcsp-workspace" aria-labelledby="bcsp-workspace-title">
          <header className="bcsp-workspace__heading">
            <div className="bcsp-workspace__identity">
              <p className="bcsp-section-label">[ {sequence} / {i18n.t('app.catalog_workspace')} ]</p>
              <div className="bcsp-workspace__title-line">
                <span aria-hidden="true" className="bcsp-workspace__sequence">{sequence}</span>
                <h2 className="bcsp-workspace__title" id="bcsp-workspace-title">
                  {workspaceTitle}
                </h2>
              </div>
              <p className="bcsp-workspace__intro">{workspaceIntro}</p>
            </div>
            <p className="bcsp-workspace__protocol">
              {i18n.t('app.protocol')} / BCSP.V{PRODUCT_PROTOCOL_VERSION}
            </p>
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
    </>
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

const SERVICE_LEVEL_KEYS = {
  INITIALIZING: 'service.level.initializing',
  PARTIALLY_READY: 'service.level.partially_ready',
  READY: 'service.level.ready',
  DEGRADED: 'service.level.degraded',
  ERROR: 'service.level.error',
} as const satisfies Record<ServiceLevelV1, Parameters<BcspI18nRuntime['t']>[0]>;

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

function targetLabel(status: ServiceStatusV1): string | null {
  const target = status.operation.target;
  return target === null ? null : `${target.term} / ${target.campus}`;
}

function ServiceStatusBand({ resource }: { readonly resource: ServiceStatusResource }) {
  const i18n = useBcspI18n();
  const status = resource.snapshot;
  const interrupted = resource.connection === 'INTERRUPTED';
  const expanded = interrupted
    || status === null
    || status.level !== 'READY'
    || status.operation.phase !== 'IDLE';
  const level = status === null
    ? i18n.t('service.level.initializing')
    : i18n.t(SERVICE_LEVEL_KEYS[status.level]);
  const phase = status === null
    ? i18n.t('service.phase.starting')
    : i18n.t(SERVICE_PHASE_KEYS[status.operation.phase]);
  const target = status === null ? null : targetLabel(status);
  const issue = status?.issues[0];
  return (
    <section
      aria-label={i18n.t('app.system_status')}
      className="bcsp-service-status"
      data-connection={resource.connection.toLowerCase()}
      data-expanded={expanded || undefined}
      data-level={status?.level.toLowerCase() ?? 'initializing'}
      id="bcsp-system-status"
    >
      <div className="bcsp-service-status__lead">
        <span className="bcsp-service-status__signal" aria-hidden="true" />
        <div>
          <p className="bcsp-service-status__kicker">{i18n.t('service.status_label')}</p>
          <p className="bcsp-service-status__headline">{interrupted
            ? i18n.t('service.connection_interrupted')
            : level}</p>
        </div>
      </div>
      <div className="bcsp-service-status__operation">
        <span className="bcsp-service-status__label">{i18n.t('service.current_operation')}</span>
        <strong>{phase}</strong>
        {target === null ? null : <samp>{target}</samp>}
      </div>
      <dl className="bcsp-service-status__counts">
        <div>
          <dt>{i18n.t('service.catalog')}</dt>
          <dd>{status === null ? '—' : `${status.catalog.availableTargetCount}/${status.catalog.totalTargetCount}`}</dd>
        </div>
        <div>
          <dt>{i18n.t('service.open')}</dt>
          <dd>{status === null ? '—' : `${status.open.availableTargetCount}/${status.open.totalTargetCount}`}</dd>
        </div>
      </dl>
      {expanded ? (
        <div className="bcsp-service-status__detail">
          <p>{interrupted
            ? i18n.t('service.connection_retained')
            : issue === undefined
              ? i18n.t('service.activity_detail')
              : `${issue.component} / ${issue.code}`}</p>
          {resource.connection === 'INTERRUPTED' ? (
            <button className="bcsp-service-status__retry" onClick={resource.retry} type="button">
              {i18n.t('action.retry')}
            </button>
          ) : null}
        </div>
      ) : null}
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
  readonly serviceStatus: ServiceStatusV1 | null;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  const { pathname } = useAppRouter();

  return (
    <>
      {pathname === '/watch'
        ? (
          <WatchWorkspace
            initialPolicy={experience.initialWatchPolicy}
            onPolicyChange={experience.onWatchPolicyChange}
          />
        )
        : (
          <SearchWorkspace
            initialFilters={experience.initialFilters}
            onFiltersChange={experience.onFiltersChange}
            runtime={runtime}
            serviceStatus={serviceStatus}
            shellState={state}
          />
        )}
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
  readonly serviceStatus: ServiceStatusV1 | null;
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
  const service = useServiceStatus(runtime);
  const { retry, state } = useShellDataState(runtime, service.revision);
  if (state.status === 'LOADING') {
    return <><ServiceStatusBand resource={service} /><InitialState detail={t('app.loading_body')} heading={t('app.loading_title')} kind="loading" /></>;
  }
  if (state.status === 'ERROR') {
    return (
      <><ServiceStatusBand resource={service} /><InitialState
          detail={t('app.catalog_error_body')}
          heading={t('app.catalog_error_title')}
          kind="error"
          retry={retry}
        /></>
    );
  }
  if (state.status === 'EMPTY') {
    return (
      <><ServiceStatusBand resource={service} /><InitialState
          detail={t('app.no_targets_body')}
          heading={t('app.no_targets_title')}
          kind="empty"
          retry={retry}
        /></>
    );
  }
  return <><ServiceStatusBand resource={service} /><ReadyCatalog
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
    <LiveWatchProvider
      initialSelected={experience.initialSelectedSections}
      initialVolume={experience.initialVolume}
      onSelectedChange={experience.onSelectedSectionsChange}
      onVolumeChange={experience.onVolumeChange}
      runtime={runtime}
    >
      <WatchWorkspaceStyles />
      {extension?.content ?? <ReadyRuntime experience={experience} runtime={runtime} />}
      <WatchToastRegion />
    </LiveWatchProvider>
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
              <ServiceStatusBand resource={BOOTSTRAP_SERVICE_RESOURCE} />
              <InitialState
                detail={i18n.t('app.loading_body')}
                heading={i18n.t('app.loading_title')}
                kind="loading"
              />
            </>
          ) : productRuntime.status === 'ERROR' ? (
            <>
              <ServiceStatusBand resource={BOOTSTRAP_ERROR_RESOURCE} />
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
