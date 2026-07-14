import type { ReactNode } from 'react';

import { ShellStyles } from './application';
import {
  ActionButton,
  DesignSystemStyles,
  Metric,
  StatePanel,
  StatusSignal,
} from './design-system';
import type { SupportedLocale } from './i18n/contract';
import { freshnessMessageKeys } from './i18n/presenter';
import { useBcspI18n, type BcspI18nRuntime } from './i18n/runtime';
import {
  PRODUCT_PROTOCOL_VERSION,
  useProductRuntimeState,
  type CatalogDiscoveryResponseV1,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionKey,
  type WatchPolicyV1,
} from './product';
import { SearchWorkspace } from './search';
import { AppRouterProvider, RouterLink, useAppRouter } from './routing';
import { useShellDataState, type ShellDataState } from './shell';
import {
  LiveWatchProvider,
  WatchToastRegion,
  WatchWorkspace,
  WatchWorkspaceStyles,
  useLiveWatch,
} from './watch';

function retryBootstrap() {
  globalThis.location?.reload();
}

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
        <aside className="bcsp-rail">
          <div className="bcsp-rail__section">
            <p aria-hidden="true" className="bcsp-rail__sequence">{sequence}</p>
            <div>
              <p className="bcsp-section-label">{i18n.t('app.catalog_index')}</p>
              <p className="bcsp-rail__note">{workspaceIntro}</p>
            </div>
          </div>
          <div className="bcsp-rail__footer">Copyright (c) 2026 VVittgenstein</div>
        </aside>
        <section className="bcsp-workspace" aria-labelledby="bcsp-workspace-title">
          <header className="bcsp-workspace__heading">
            <div>
              <p className="bcsp-section-label">[ {sequence} / {i18n.t('app.catalog_workspace')} ]</p>
              <h2 className="bcsp-workspace__title" id="bcsp-workspace-title">
                {workspaceTitle}
              </h2>
            </div>
            <p className="bcsp-workspace__protocol">
              {i18n.t('app.protocol')} / BCSP.V{PRODUCT_PROTOCOL_VERSION}
            </p>
          </header>
          {children}
        </section>
      </main>
      <footer className="bcsp-mobile-footer">
        <span className="bcsp-mobile-footer__copyright">
          Copyright (c) 2026 VVittgenstein
        </span>
        <span className="bcsp-mobile-footer__protocol">
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

function statusTime(discovery: CatalogDiscoveryResponseV1, i18n: BcspI18nRuntime): string {
  const observedAt = discovery.status.lastSuccess?.observedAt;
  if (observedAt === undefined) return i18n.t('freshness.never_observed');
  const value = Date.parse(observedAt);
  if (!Number.isFinite(value)) return i18n.t('freshness.unknown');
  return i18n.formatDate(value, { dateStyle: 'medium', timeStyle: 'short' });
}

function ReadyCatalogContent({
  experience,
  runtime,
  state,
}: {
  readonly experience: SharedExperienceConfiguration;
  readonly runtime: ProductRuntimePort;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  const i18n = useBcspI18n();
  const discoveryLabel = state.discoveryState === 'CURRENT'
    ? i18n.t('app.data_current')
    : i18n.t('app.data_stale');
  const discoverySignal = state.discoveryState === 'CURRENT' ? 'ready' : 'stale';
  const { pathname } = useAppRouter();
  const watch = useLiveWatch();
  const openStatus = watch.batchStatuses[0];
  const openSignal = openStatus === undefined
    ? 'unknown'
    : openStatus.freshness.state === 'FRESH'
      ? 'ready'
      : 'stale';
  const openDetail = openStatus === undefined
    ? i18n.t('freshness.never_observed')
    : `${openStatus.scheduler.schedulerLagMilliseconds}ms / ${openStatus.scheduler.requestedEffectiveIntervalSeconds}s`;

  return (
    <>
      <section
        aria-label={i18n.t('app.system_status')}
        className="bcsp-status-grid"
        id="bcsp-system-status"
      >
        <div>
          <StatusSignal
            detail={statusTime(state.discovery, i18n)}
            label={discoveryLabel}
            state={discoverySignal}
          />
        </div>
        <Metric
          label={i18n.t('app.targets')}
          value={i18n.formatNumber(state.discovery.targets.length)}
        />
        <Metric
          label={i18n.t('app.filter_definitions')}
          value={i18n.formatNumber(state.filterCount)}
        />
        <div>
          <StatusSignal
            detail={openDetail}
            label={openStatus === undefined
              ? i18n.t('freshness.lag')
              : i18n.t(freshnessMessageKeys[openStatus.freshness.state])}
            state={openSignal}
          />
        </div>
      </section>
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
            shellState={state}
          />
        )}
    </>
  );
}

function ReadyCatalog({
  experience,
  runtime,
  state,
}: {
  readonly runtime: ProductRuntimePort;
  readonly experience: SharedExperienceConfiguration;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  return (
    <ReadyCatalogContent experience={experience} runtime={runtime} state={state} />
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
  const { retry, state } = useShellDataState(runtime);
  if (state.status === 'LOADING') {
    return <InitialState detail={t('app.loading_body')} heading={t('app.loading_title')} kind="loading" />;
  }
  if (state.status === 'ERROR') {
    return (
      <InitialState
        detail={t('app.catalog_error_body')}
        heading={t('app.catalog_error_title')}
        kind="error"
        retry={retry}
      />
    );
  }
  if (state.status === 'EMPTY') {
    return (
      <InitialState
        detail={t('app.no_targets_body')}
        heading={t('app.no_targets_title')}
        kind="empty"
        retry={retry}
      />
    );
  }
  return <ReadyCatalog experience={experience} runtime={runtime} state={state} />;
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
            <InitialState
              detail={i18n.t('app.loading_body')}
              heading={i18n.t('app.loading_title')}
              kind="loading"
            />
          ) : productRuntime.status === 'ERROR' ? (
            <InitialState
              detail={i18n.t('app.bootstrap_error_body')}
              heading={i18n.t('app.bootstrap_error_title')}
              kind="error"
              retry={retryBootstrap}
            />
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
