import type { SectionKey } from '../product';
import { useBcspI18n } from '../i18n/runtime';
import { useLiveWatchOptional } from './LiveWatchProvider';

export function SectionSelectionAction({ sectionKey }: { readonly sectionKey: SectionKey }) {
  const i18n = useBcspI18n();
  const watch = useLiveWatchOptional();
  if (watch === null) return null;
  const selected = watch.isSelected(sectionKey);
  const active = watch.isActive(sectionKey);
  const pending = watch.pending.some((value) =>
    value.term === sectionKey.term
    && value.campus === sectionKey.campus
    && value.index === sectionKey.index);
  return (
    <button
      aria-label={i18n.t('watch.selection_label', {
        action: i18n.t(selected ? 'watch.selection.remove' : 'watch.selection.select'),
        index: sectionKey.index,
      })}
      aria-pressed={selected}
      className="watch-selection-action"
      disabled={active || pending}
      onClick={() => selected ? watch.remove(sectionKey) : watch.select(sectionKey)}
      type="button"
    >
      {active
        ? i18n.t('watch.state.watching')
        : pending
          ? i18n.t('watch.state.starting')
          : selected
            ? i18n.t('watch.state.selected')
            : i18n.t('watch.selection.action')}
    </button>
  );
}
