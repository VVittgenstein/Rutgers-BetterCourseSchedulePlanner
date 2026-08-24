import type { SectionKey } from '../product';
import { useBcspI18n } from '../i18n/runtime';
import { RouterLink } from '../routing';
import { intentStateMessageKeys } from './intentLabels';
import { useLiveWatchOptional } from './LiveWatchProvider';

export function SectionSelectionAction({ sectionKey }: { readonly sectionKey: SectionKey }) {
  const i18n = useBcspI18n();
  const watch = useLiveWatchOptional();
  if (watch === null) return null;
  const selected = watch.isSelected(sectionKey);
  const active = watch.isActive(sectionKey);
  const watchable = watch.isWatchable(sectionKey);
  const pending = watch.pending.some((value) =>
    value.term === sectionKey.term
    && value.campus === sectionKey.campus
    && value.index === sectionKey.index);
  // With durable intent, whether this button may take the Section off the
  // list is the SERVER's answer, not this page's. `active` is built from the
  // START frames this page happened to receive, so on a page opened after the
  // watches were armed it is empty while the process is watching -- and a
  // button that trusted it would quietly remove the row that carries the only
  // STOP control. When the intent could not be read at all the answer is "not
  // now": guessing here is the one thing that hides a running watch.
  const intentEnabled = watch.intentStatus !== 'DISABLED';
  const removable = watch.isRemovable(sectionKey);
  const blocked = selected && !removable;
  // The SAME five states the watch desk uses, from the same derivation.
  //
  // The earlier shape here said "watching" for any Section with a policy,
  // which is a claim about what the user ASKED for read out as a claim about
  // what is running. Every state that is not a complete four-part stamp match
  // -- preparing, stopping, needs attention -- appeared on the search page as
  // a green light, on the one screen where a user decides whether they still
  // need to do something about a Section.
  const intentState = intentEnabled ? watch.intentStateFor(sectionKey) : null;
  const label = intentEnabled && watch.intentStatus === 'FAILED'
    ? i18n.t('watch.intent.unavailable')
    : intentEnabled && watch.intentStatus === 'LOADING'
      ? i18n.t('watch.intent.loading')
      : intentState !== null && intentState !== 'NOT_WATCHING'
        ? i18n.t(intentStateMessageKeys[intentState])
        : active
          ? i18n.t('watch.state.watching')
          : pending
            ? i18n.t('watch.state.starting')
            : !watchable && !selected
              ? i18n.t('watch.term_out_of_range')
              : selected
                ? i18n.t('watch.state.selected')
                : i18n.t('watch.selection.action');
  return (
    <div className="watch-selection-control">
      <button
        aria-label={i18n.t(selected ? 'watch.selection_remove_label' : 'watch.selection_add_label', {
          index: sectionKey.index,
        })}
        aria-pressed={selected}
        className="watch-selection-action"
        disabled={active || pending || blocked || (!selected && !watchable)}
        onClick={() => selected ? watch.remove(sectionKey) : watch.select(sectionKey)}
        type="button"
      >
        {label}
      </button>
      {blocked || (intentState !== null && intentState !== 'NOT_WATCHING') ? (
        <RouterLink className="watch-selection-control__link" to="/watch">
          {i18n.t('watch.selection.go_to_desk')}
        </RouterLink>
      ) : selected && !active && !pending ? (
        <RouterLink className="watch-selection-control__link" to="/watch">
          {i18n.t('watch.selection.go_to_desk')}
        </RouterLink>
      ) : null}
    </div>
  );
}
