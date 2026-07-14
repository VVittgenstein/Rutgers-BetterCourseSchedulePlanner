import type { ReactNode } from 'react';

import { ShellStyles } from './application';
import {
  ActionButton,
  DesignSystemStyles,
  Metric,
  StatePanel,
  StatusSignal,
} from './design-system';
import { useBcspI18n, type BcspI18nRuntime } from './i18n/runtime';
import {
  PRODUCT_PROTOCOL_VERSION,
  useProductRuntimeState,
  type CatalogDiscoveryResponseV1,
  type ProductRuntimePort,
} from './product';
import { SearchWorkspace } from './search';
import { AppRouterProvider, RouterLink, useAppRouter } from './routing';
import { useShellDataState, type ShellDataState } from './shell';

function retryBootstrap() {
  globalThis.location?.reload();
}

function LanguageControl({ i18n }: { readonly i18n: BcspI18nRuntime }) {
  return (
    <fieldset className="bcsp-language">
      <legend className="bcsp-visually-hidden">{i18n.t('app.locale_control')}</legend>
      <button
        aria-pressed={i18n.locale === 'en-US'}
        className="bcsp-language__button"
        onClick={() => void i18n.changeLocale('en-US')}
        type="button"
      >
        EN / US
      </button>
      <button
        aria-pressed={i18n.locale === 'zh-CN'}
        className="bcsp-language__button"
        onClick={() => void i18n.changeLocale('zh-CN')}
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
}: {
  readonly children: ReactNode;
  readonly i18n: BcspI18nRuntime;
}) {
  const { pathname } = useAppRouter();
  const sectionWorkspace = pathname.startsWith('/sections');
  const directSection = sectionWorkspace && pathname !== '/sections';
  const sequence = sectionWorkspace ? '02' : '01';
  const workspaceTitle = directSection
    ? i18n.t('search.section_detail_title')
    : sectionWorkspace
      ? i18n.t('search.section_workspace')
      : i18n.t('search.course_workspace');
  const workspaceIntro = sectionWorkspace
    ? i18n.t('search.section_intro')
    : i18n.t('search.course_intro');
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
          <LanguageControl i18n={i18n} />
        </div>
      </header>
      <nav aria-label={i18n.t('app.navigation')} className="bcsp-navigation">
        <span className="bcsp-navigation__label">[ {i18n.t('app.catalog_workspace')} ]</span>
        <RouterLink
          className="bcsp-navigation__link"
          data-active={!sectionWorkspace || undefined}
          to="/"
        >
          <span>01</span>{i18n.t('app.nav_courses')}
        </RouterLink>
        <RouterLink
          className="bcsp-navigation__link"
          data-active={sectionWorkspace || undefined}
          to="/sections"
        >
          <span>02</span>{i18n.t('app.nav_sections')}
        </RouterLink>
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

function ReadyCatalog({
  runtime,
  state,
}: {
  readonly runtime: ProductRuntimePort;
  readonly state: Extract<ShellDataState, { status: 'READY' }>;
}) {
  const i18n = useBcspI18n();
  const discoveryLabel = state.discoveryState === 'CURRENT'
    ? i18n.t('app.data_current')
    : i18n.t('app.data_stale');
  const discoverySignal = state.discoveryState === 'CURRENT' ? 'ready' : 'stale';

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
            detail={i18n.t('freshness.never_observed')}
            label={i18n.t('freshness.lag')}
            state="unknown"
          />
        </div>
      </section>
      <SearchWorkspace runtime={runtime} shellState={state} />
    </>
  );
}

function ReadyRuntime({ runtime }: { readonly runtime: ProductRuntimePort }) {
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
  return <ReadyCatalog runtime={runtime} state={state} />;
}

export function SharedApplication() {
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
        <ShellFrame i18n={i18n}>
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
            <ReadyRuntime runtime={productRuntime.runtime} />
          )}
        </ShellFrame>
      </div>
    </AppRouterProvider>
  );
}
