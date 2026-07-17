// @vitest-environment jsdom

import { useState, type ComponentProps } from 'react';
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SavedViewsPage } from '../src/ui/local/pages/SavedViewsPage';
import { SettingsPage } from '../src/ui/local/pages/SettingsPage';
import type {
  LocalSettings,
  SavedViewDefinition,
  SavedViewLibrary,
  StoredSettings,
} from '../src/ui/local/personal';
import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import { AppRouterProvider } from '../src/ui/shared/routing';

const SETTINGS: LocalSettings = {
  localeOverride: 'system',
  catalogRefreshMinutes: 60,
  openRefreshSeconds: 30,
  watchFastLaneSeconds: 10,
  volumePercent: 70,
  soundPolicy: {
    notificationMode: 'ONE_SHOT',
    maxAudible: 3,
    continuousDuration: { kind: 'FINITE', seconds: 600 },
  },
};

const STORED_SETTINGS: StoredSettings = { revision: 4, value: SETTINGS };

const VIEW = {
  id: '10000000-0000-4000-8000-000000000001',
  name: 'Morning CS options',
  schemaVersion: 1,
  revision: 2,
  content: {
    status: 'COMPATIBLE',
    filters: { contractVersion: 2, values: {} },
  },
  createdAt: Date.parse('2026-07-14T08:00:00Z'),
  updatedAt: Date.parse('2026-07-15T08:00:00Z'),
} as SavedViewDefinition;

function library(views: readonly SavedViewDefinition[] = [VIEW]): SavedViewLibrary {
  return {
    stateRevision: 9,
    currentFilters: {
      stateRevision: 9,
      revision: 3,
      value: {
        association: { kind: 'CUSTOM' },
        content: VIEW.content,
      },
    },
    views: views.map((definition) => ({ definition, matchState: 'NOT_APPLIED' })),
  };
}

const noop = vi.fn(async () => undefined);

function renderSettings(overrides: Partial<ComponentProps<typeof SettingsPage>> = {}) {
  return render(
    <BcspI18nProvider initialLocale="en-US">
      <SettingsPage
        onConfirmUserDataReset={noop}
        onDeleteAllSavedViews={noop}
        onPrepareUserDataReset={async () => ({
          confirmationToken: '20000000-0000-4000-8000-000000000001',
          expectedUserStateRevision: 9,
          expiresInSeconds: 60,
        })}
        onResetCurrentFilters={noop}
        onUpdateSettings={noop}
        savedViewCount={1}
        settings={STORED_SETTINGS}
        {...overrides}
      />
    </BcspI18nProvider>,
  );
}

interface SavedHarnessProps {
  readonly deleteAll?: boolean;
  readonly deleteOne?: boolean;
  readonly views?: readonly SavedViewDefinition[];
}

function SavedHarness({ deleteAll = false, deleteOne = false, views }: SavedHarnessProps) {
  const [current, setCurrent] = useState(() => library(views));
  return (
    <BcspI18nProvider initialLocale="en-US">
      <AppRouterProvider initialPath="/saved-views">
        <SavedViewsPage
          library={current}
          onApply={noop}
          onCreate={noop}
          onDelete={async () => {
            if (deleteOne) setCurrent(library([]));
          }}
          onDeleteAll={async () => {
            if (deleteAll) setCurrent(library([]));
          }}
          onDuplicate={noop}
          onRename={noop}
          onUpdate={noop}
        />
      </AppRouterProvider>
    </BcspI18nProvider>
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('P7.3-002 local interaction lifecycle', () => {
  it('defaults the watch fast lane to 10 seconds and accepts only the 3–60 boundary range', async () => {
    const updateSettings = vi.fn(async () => undefined);
    renderSettings({ onUpdateSettings: updateSettings });
    const input = screen.getByRole('spinbutton', {
      name: 'Watch fast-lane interval (seconds)',
    }) as HTMLInputElement;
    const save = screen.getByRole('button', { name: 'Save settings' }) as HTMLButtonElement;

    expect(input.value).toBe('10');
    expect(input.min).toBe('3');
    expect(input.max).toBe('60');
    for (const invalid of ['2', '61']) {
      fireEvent.change(input, { target: { value: invalid } });
      expect(save.disabled).toBe(true);
    }
    for (const valid of ['3', '60']) {
      fireEvent.change(input, { target: { value: valid } });
      expect(save.disabled).toBe(false);
      fireEvent.click(save);
      await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ watchFastLaneSeconds: Number(valid) }),
      ));
    }
  });

  it('distinguishes every Settings save state and disables no-op saves', async () => {
    let finishSave!: () => void;
    const firstSave = new Promise<void>((resolve) => {
      finishSave = resolve;
    });
    const updateSettings = vi.fn()
      .mockReturnValueOnce(firstSave)
      .mockRejectedValueOnce(new Error('disk unavailable'));
    renderSettings({ onUpdateSettings: updateSettings });

    const save = screen.getByRole('button', { name: 'Save settings' }) as HTMLButtonElement;
    const state = screen.getByText('NO UNSAVED CHANGES');
    const catalog = screen.getByRole('spinbutton', {
      name: 'Catalog refresh interval (minutes)',
    });

    expect(state.getAttribute('data-state')).toBe('UNCHANGED');
    expect(save.disabled).toBe(true);

    fireEvent.change(catalog, { target: { value: '0' } });
    expect(state.getAttribute('data-state')).toBe('INVALID');
    expect(save.disabled).toBe(true);

    fireEvent.change(catalog, { target: { value: '61' } });
    expect(state.getAttribute('data-state')).toBe('DIRTY');
    expect(save.disabled).toBe(false);

    fireEvent.click(save);
    expect(state.getAttribute('data-state')).toBe('SAVING');
    finishSave();
    await waitFor(() => expect(state.getAttribute('data-state')).toBe('SAVED'));
    expect(save.disabled).toBe(true);

    fireEvent.change(catalog, { target: { value: '62' } });
    expect(state.getAttribute('data-state')).toBe('DIRTY');
    fireEvent.click(save);
    await waitFor(() => expect(state.getAttribute('data-state')).toBe('FAILED'));
    expect(state.getAttribute('aria-live')).toBe('assertive');
    expect(save.disabled).toBe(false);
  });

  it('moves focus into Settings confirmations and returns it when they close', async () => {
    renderSettings();

    const filterTrigger = screen.getByRole('button', { name: 'Reset filters only' });
    fireEvent.click(filterTrigger);
    const filterConfirm = screen.getByRole('button', { name: 'Reset filters only' });
    expect(document.activeElement).toBe(filterConfirm);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Reset filters only' }));

    const fullTrigger = screen.getByRole('button', { name: 'Prepare full reset' });
    fireEvent.click(fullTrigger);
    const fullConfirm = await screen.findByRole('button', { name: 'Confirm full local reset' });
    expect(document.activeElement).toBe(fullConfirm);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Prepare full reset' }));
  });

  it('returns Saved view edit and confirmation focus to their triggers', () => {
    render(<SavedHarness />);

    const rename = screen.getByRole('button', { name: 'Rename: Morning CS options' });
    fireEvent.click(rename);
    const editor = screen.getAllByRole('textbox', { name: 'New view name' })
      .find((input) => (input as HTMLInputElement).value === 'Morning CS options');
    expect(document.activeElement).toBe(editor);
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(document.activeElement).toBe(rename);

    const remove = screen.getByRole('button', { name: 'Delete: Morning CS options' });
    fireEvent.click(remove);
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Confirm delete' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(document.activeElement).toBe(remove);
  });

  it('shows localized Saved-view migration fields while keeping stable IDs diagnostic-only', () => {
    const reviewView = {
      ...VIEW,
      id: '10000000-0000-4000-8000-000000000002',
      name: 'Legacy subject view',
      content: {
        status: 'REVIEW_REQUIRED',
        rawSnapshot: { codecVersion: 2 },
        reasons: [{ stableId: 'FLT-C03', code: 'DYNAMIC_VALUE_UNAVAILABLE' }],
      },
    } as SavedViewDefinition;
    render(<SavedHarness views={[reviewView]} />);

    const field = screen.getByText('Subject');
    expect(field.getAttribute('title')).toBe('FLT-C03');
    expect(document.body.textContent).not.toContain('FLT-C03');
  });

  it('lands on the stable library heading after delete and delete-all remove their triggers', async () => {
    const first = render(<SavedHarness deleteOne />);
    fireEvent.click(screen.getByRole('button', { name: 'Delete: Morning CS options' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete' }));
    await waitFor(() => expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: 'View library' }),
    ));
    first.unmount();

    render(<SavedHarness deleteAll />);
    fireEvent.click(screen.getByRole('button', { name: 'Delete all Saved views' }));
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Confirm delete all' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete all' }));
    await waitFor(() => expect(document.activeElement).toBe(
      screen.getByRole('heading', { name: 'View library' }),
    ));
  });
});
