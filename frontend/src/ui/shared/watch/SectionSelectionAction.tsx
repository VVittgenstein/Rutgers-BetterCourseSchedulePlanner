import type { SectionKey } from '../product';
import { useLiveWatchOptional } from './LiveWatchProvider';

export function SectionSelectionAction({ sectionKey }: { readonly sectionKey: SectionKey }) {
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
      aria-label={`${selected ? 'Remove' : 'Select'} Section ${sectionKey.index} for watch`}
      aria-pressed={selected}
      className="watch-selection-action"
      disabled={active || pending}
      onClick={() => selected ? watch.remove(sectionKey) : watch.select(sectionKey)}
      type="button"
    >
      {active ? 'Watching' : pending ? 'Starting' : selected ? 'Selected' : '+ Watch'}
    </button>
  );
}
