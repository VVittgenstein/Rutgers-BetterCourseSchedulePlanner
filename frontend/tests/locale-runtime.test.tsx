// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { LocalCompositionRoot } from '../src/ui/local/LocalCompositionRoot';
import {
  resolveLocalLocale,
  resolveLocalLocaleAfterReset,
} from '../src/ui/local/i18n/localeBootstrap';
import { PublicCompositionRoot } from '../src/ui/public/PublicCompositionRoot';
import {
  fallbackLocale,
  lookupMessage,
  messages,
  type MessageKey,
  type SupportedLocale,
} from '../src/ui/shared/i18n/contract';
import { BcspI18nProvider, useBcspI18n } from '../src/ui/shared/i18n/runtime';

afterEach(() => {
  cleanup();
  document.documentElement.lang = '';
  vi.restoreAllMocks();
});

function firstMessageKey(): MessageKey {
  const key = Object.keys(messages[fallbackLocale])[0];
  if (key === undefined) throw new Error('the locale catalog must not be empty');
  return key as MessageKey;
}

function RuntimeProbe({ id }: { readonly id: string }) {
  const { changeLocale, formatDate, formatNumber, locale, t } = useBcspI18n();
  const key = firstMessageKey();
  const date = Date.UTC(2026, 6, 14, 12);
  const nextLocale: SupportedLocale = locale === 'en-US' ? 'zh-CN' : 'en-US';
  return (
    <div>
      <output data-testid={`${id}-locale`}>{locale}</output>
      <output data-testid={`${id}-message`}>{t(key)}</output>
      <output data-testid={`${id}-number`}>{formatNumber(12_345.6)}</output>
      <output data-testid={`${id}-date`}>
        {formatDate(date, { dateStyle: 'medium', timeZone: 'UTC' })}
      </output>
      <button type="button" onClick={() => void changeLocale(nextLocale)}>
        {`change-${id}`}
      </button>
    </div>
  );
}

describe('locale bootstraps', () => {
  it('negotiates the public locale without reading browser storage', () => {
    const storageProperties = ['localStorage', 'sessionStorage'] as const;
    const descriptors = storageProperties.map((property) => [
      property,
      Object.getOwnPropertyDescriptor(window, property),
    ] as const);
    const accessed = vi.fn();
    for (const property of storageProperties) {
      Object.defineProperty(window, property, {
        configurable: true,
        get() {
          accessed(property);
          throw new Error('public locale bootstrap must not read storage');
        },
      });
    }

    try {
      const { container } = render(
        <PublicCompositionRoot localeSource={{ languages: ['zh-CN', 'en-US'] }} />,
      );
      expect(
        container.querySelector('[data-bcsp-shared-application]')?.getAttribute('data-bcsp-locale'),
      ).toBe('zh-CN');
      expect(accessed).not.toHaveBeenCalled();
    } finally {
      for (const [property, descriptor] of descriptors) {
        if (descriptor !== undefined) Object.defineProperty(window, property, descriptor);
        else Reflect.deleteProperty(window, property);
      }
    }
  });

  it('uses a valid injected local locale, ignores invalid input, and redetects after reset', () => {
    expect(
      resolveLocalLocale({ savedLocale: 'zh-CN', systemLanguages: ['en-US'] }),
    ).toBe('zh-CN');
    expect(
      resolveLocalLocale({ savedLocale: 'not-a-locale', systemLanguages: ['en-US'] }),
    ).toBe('en-US');
    expect(resolveLocalLocaleAfterReset(['zh-CN'])).toBe('zh-CN');

    const { container } = render(
      <LocalCompositionRoot savedLocale="zh-CN" systemLanguages={['en-US']} />,
    );
    expect(
      container.querySelector('[data-bcsp-shared-application]')?.getAttribute('data-bcsp-locale'),
    ).toBe('zh-CN');
  });
});

describe('locale runtime', () => {
  it('syncs html lang, changes locale, translates, and formats through the active locale', async () => {
    const changes = vi.fn();
    const key = firstMessageKey();
    render(
      <BcspI18nProvider initialLocale="en-US" onLocaleChange={changes}>
        <RuntimeProbe id="runtime" />
      </BcspI18nProvider>,
    );

    expect(document.documentElement.lang).toBe('en-US');
    expect(screen.getByTestId('runtime-message').textContent).toBe(lookupMessage('en-US', key));
    expect(screen.getByTestId('runtime-number').textContent).toBe(
      new Intl.NumberFormat('en-US').format(12_345.6),
    );

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'change-runtime' }));
    });
    await waitFor(() => expect(screen.getByTestId('runtime-locale').textContent).toBe('zh-CN'));

    expect(document.documentElement.lang).toBe('zh-CN');
    expect(changes).toHaveBeenCalledOnce();
    expect(changes).toHaveBeenCalledWith('zh-CN');
    expect(screen.getByTestId('runtime-message').textContent).toBe(lookupMessage('zh-CN', key));
    expect(screen.getByTestId('runtime-number').textContent).toBe(
      new Intl.NumberFormat('zh-CN').format(12_345.6),
    );
    expect(screen.getByTestId('runtime-date').textContent).toBe(
      new Intl.DateTimeFormat('zh-CN', { dateStyle: 'medium', timeZone: 'UTC' }).format(
        Date.UTC(2026, 6, 14, 12),
      ),
    );
  });

  it('creates isolated runtime state for separate provider mounts', async () => {
    render(
      <>
        <BcspI18nProvider initialLocale="en-US">
          <RuntimeProbe id="first" />
        </BcspI18nProvider>
        <BcspI18nProvider initialLocale="en-US">
          <RuntimeProbe id="second" />
        </BcspI18nProvider>
      </>,
    );

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'change-first' }));
    });
    await waitFor(() => expect(screen.getByTestId('first-locale').textContent).toBe('zh-CN'));
    expect(screen.getByTestId('second-locale').textContent).toBe('en-US');
  });
});
