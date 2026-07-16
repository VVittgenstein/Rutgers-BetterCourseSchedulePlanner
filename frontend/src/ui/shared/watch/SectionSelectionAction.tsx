import type { SectionKey } from '../product';
import { useBcspI18n } from '../i18n/runtime';
import { RouterLink } from '../routing';
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
    <div className="watch-selection-control">
      <button
        aria-label={i18n.t(selected ? 'watch.selection_remove_label' : 'watch.selection_add_label', {
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
      {selected && !active && !pending ? (
        <RouterLink className="watch-selection-control__link" to="/watch">
          {i18n.t('watch.selection.go_to_desk')}
        </RouterLink>
      ) : null}
    </div>
  );
}
