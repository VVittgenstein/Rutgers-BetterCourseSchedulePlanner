// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SharedApplication } from '../src/ui/shared/SharedApplication';
import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import { ProductRuntimeProvider } from '../src/ui/shared/product';
import { AppRouterProvider, RouterLink, useAppRouter } from '../src/ui/shared/routing';

afterEach(() => {
  cleanup();
  window.history.replaceState(null, '', '/');
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

function RouterProbe() {
  const { pathname } = useAppRouter();
  return (
    <>
      <RouterLink to="/watch">Forward</RouterLink>
      <button type="button">Keep focus</button>
      <main id="bcsp-workspace" tabIndex={-1}>{pathname}</main>
    </>
  );
}

describe('workspace navigation focus', () => {
  it('marks only the active destination and focuses the workspace after an in-app route', async () => {
    window.history.replaceState(null, '', '/watch');
    render(
      <BcspI18nProvider initialLocale="en-US">
        <ProductRuntimeProvider state={{ status: 'LOADING' }}>
          <SharedApplication />
        </ProductRuntimeProvider>
      </BcspI18nProvider>,
    );

    const workspace = document.getElementById('bcsp-workspace') as HTMLElement;
    expect(document.activeElement).not.toBe(workspace);
    expect(screen.getByRole('link', { name: /Watch desk/u }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByText(/Select up to nine Sections/u)).toBeTruthy();
    expect(screen.getAllByText('Copyright (c) 2026 VVittgenstein')).toHaveLength(2);

    fireEvent.click(screen.getByRole('link', { name: /Courses/u }));

    await waitFor(() => expect(document.activeElement).toBe(workspace));
    expect(screen.getByRole('link', { name: /Courses/u }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('link', { name: /Watch desk/u }).hasAttribute('aria-current')).toBe(false);
  });

  it('leaves focus and scroll restoration alone on initial render and popstate', async () => {
    window.history.replaceState(null, '', '/');
    const view = render(
      <AppRouterProvider>
        <RouterProbe />
      </AppRouterProvider>,
    );
    const workspace = view.container.querySelector<HTMLElement>('#bcsp-workspace')!;
    const scrollIntoView = vi.fn();
    Object.defineProperty(workspace, 'scrollIntoView', { configurable: true, value: scrollIntoView });
    const focusKeeper = screen.getByRole('button', { name: 'Keep focus' });
    focusKeeper.focus();

    expect(document.activeElement).toBe(focusKeeper);
    expect(scrollIntoView).not.toHaveBeenCalled();

    window.history.pushState(null, '', '/sections');
    window.dispatchEvent(new PopStateEvent('popstate'));

    await screen.findByText('/sections');
    expect(document.activeElement).toBe(focusKeeper);
    expect(scrollIntoView).not.toHaveBeenCalled();
  });
});
