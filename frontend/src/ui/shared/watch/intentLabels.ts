import type { MessageKey } from '../i18n/contract';
import type { WatchIntentState } from './intent';

/**
 * The one place a watch intent state becomes words.
 *
 * Shared rather than repeated because the two screens that render it -- the
 * watch desk and the search result entry -- are describing the same fact, and
 * the search entry is where a second, looser mapping did real damage: any
 * Section with a policy read as "watching", so preparing, stopping and needs-
 * attention all appeared as a green light on the screen a user looks at to
 * decide whether anything still needs doing.
 *
 * Exhaustive by type. A new state cannot be added without deciding what it
 * says here.
 */
export const intentStateMessageKeys = {
  NOT_WATCHING: 'watch.intent.not_watching',
  PREPARING: 'watch.intent.preparing',
  WATCHING: 'watch.intent.watching',
  STOPPING: 'watch.intent.stopping',
  ATTENTION: 'watch.intent.attention',
} as const satisfies Readonly<Record<WatchIntentState, MessageKey>>;
