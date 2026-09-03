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
      <nav className="bcsp-navigation"><RouterLink to="/watch">Forward</RouterLink></nav>
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
    expect(screen.queryByRole('link', { name: /Sections/u })).toBeNull();
    expect(screen.getByText(/Select up to 9 Sections/u)).toBeTruthy();
    expect(screen.getAllByText('Copyright (c) 2026 VVittgenstein')).toHaveLength(1);

    fireEvent.click(screen.getByRole('link', { name: /Courses/u }));

    await waitFor(() => expect(document.activeElement).toBe(workspace));
    expect(screen.getByRole('link', { name: /Courses/u }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('link', { name: /Watch desk/u }).hasAttribute('aria-current')).toBe(false);
  });

  it('leaves initial focus alone, then focuses the workspace and restores per-page scroll', async () => {
    window.history.replaceState(null, '', '/');
    const view = render(
      <AppRouterProvider>
        <RouterProbe />
      </AppRouterProvider>,
    );
    const workspace = view.container.querySelector<HTMLElement>('#bcsp-workspace')!;
    const navigation = view.container.querySelector<HTMLElement>('.bcsp-navigation')!;
    const scrollIntoView = vi.fn();
    Object.defineProperty(navigation, 'scrollIntoView', { configurable: true, value: scrollIntoView });
    const scrollTo = vi.fn();
    Object.defineProperty(window, 'scrollTo', { configurable: true, value: scrollTo });
    Object.defineProperty(window, 'scrollY', { configurable: true, value: 320 });
    const focusKeeper = screen.getByRole('button', { name: 'Keep focus' });
    focusKeeper.focus();

    expect(document.activeElement).toBe(focusKeeper);
    expect(scrollIntoView).not.toHaveBeenCalled();

    window.history.pushState(null, '', '/watch');
    window.dispatchEvent(new PopStateEvent('popstate'));

    await screen.findByText('/watch');
    await waitFor(() => expect(document.activeElement).toBe(workspace));
    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: 'auto', block: 'start' });

    Object.defineProperty(window, 'scrollY', { configurable: true, value: 44 });
    window.history.pushState(null, '', '/');
    window.dispatchEvent(new PopStateEvent('popstate'));
    await screen.findByText('/');
    await waitFor(() => expect(scrollTo).toHaveBeenCalledWith({
      behavior: 'auto',
      left: 0,
      top: 320,
    }));
  });

  it('keeps an independent window scroll position for every top-level workspace', async () => {
    window.history.replaceState(null, '', '/');
    render(<AppRouterProvider><RouterProbe /></AppRouterProvider>);
    const navigation = document.querySelector<HTMLElement>('.bcsp-navigation');
    if (navigation === null) throw new Error('navigation is required');
    Object.defineProperty(navigation, 'scrollIntoView', {
      configurable: true,
      value: vi.fn(),
    });
    const scrollTo = vi.fn();
    Object.defineProperty(window, 'scrollTo', { configurable: true, value: scrollTo });
    let currentScroll = 111;
    Object.defineProperty(window, 'scrollY', {
      configurable: true,
      get: () => currentScroll,
    });
    const pages = [
      ['/', 111],
      ['/watch', 222],
      ['/saved-views', 333],
      ['/history', 444],
      ['/settings', 555],
    ] as const;

    for (const [path, position] of pages.slice(1)) {
      window.history.pushState(null, '', path);
      window.dispatchEvent(new PopStateEvent('popstate'));
      await screen.findByText(path);
      currentScroll = position;
    }

    scrollTo.mockClear();
    for (const [path, position] of pages.slice(0, -1)) {
      window.history.pushState(null, '', path);
      window.dispatchEvent(new PopStateEvent('popstate'));
      await screen.findByText(path);
      await waitFor(() => expect(scrollTo).toHaveBeenLastCalledWith({
        behavior: 'auto',
        left: 0,
        top: position,
      }));
      currentScroll = position;
    }
  });
});
