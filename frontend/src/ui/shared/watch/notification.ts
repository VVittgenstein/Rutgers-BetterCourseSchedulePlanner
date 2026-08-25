/**
 * Page-level system notifications, and nothing else.
 *
 * The one capability declared here is `current-page-notification`: a
 * notification posted by THIS running page, through the browser, with the
 * user's permission. Server push, native notifications, service-worker
 * notifications and Web Push all remain denied on both targets -- a page that
 * is not running cannot make an honest claim about what is being watched, and
 * this surface exists precisely to stop dishonest claims.
 */

export type WatchNotificationPermission = 'granted' | 'denied' | 'default' | 'unsupported';

/** What the page has to say, independent of how any locale words it. */
export type WatchNotificationKind =
  /** A watched Section opened while the page could not be heard. */
  | 'SECTION_OPEN'
  /** The sound for an alert the page already showed did not play. */
  | 'CUE_FAILED'
  /** Recovery has been failing long enough that the user should know. */
  | 'MONITORING_DEGRADED';

export interface WatchNotificationRequest {
  readonly id: number;
  readonly kind: WatchNotificationKind;
  /**
   * The alert or episode this is about, or `null` for a page-wide statement.
   *
   * Also the deduplication key's subject: at most one notification is ever
   * posted for one alert, however many ways the page learns it could not be
   * heard.
   */
  readonly subject: string | null;
  readonly sectionKey?: { readonly term: string; readonly campus: string; readonly index: string } | undefined;
}

/** The browser seam. Everything the page needs, and nothing it does not. */
export interface WatchNotificationPort {
  readonly permission: WatchNotificationPermission;
  /**
   * Asks the browser for permission, and answers when the user has.
   *
   * The CALL has to happen inside the user gesture that is still running --
   * an `await` before it, even on an already-resolved promise, ends the
   * activation and the browser refuses. Awaiting the answer afterwards is
   * fine, and is how the page learns what was chosen.
   */
  requestPermission(): Promise<WatchNotificationPermission>;
  show(title: string, body: string, tag: string): void;
}

export interface BrowserNotificationConstructor {
  new (title: string, options?: { body?: string; tag?: string }): unknown;
  readonly permission: string;
  requestPermission(): Promise<string>;
}

function readPermission(value: string | undefined): WatchNotificationPermission {
  return value === 'granted' || value === 'denied' || value === 'default' ? value : 'unsupported';
}

/**
 * The real browser port, or an inert one where the API does not exist.
 *
 * `unsupported` is a first-class answer rather than an error: readiness has to
 * be able to say "this page cannot fall back to a notification", and a throw
 * would instead take out whatever was asking.
 */
export function createBrowserNotificationPort(
  source?: BrowserNotificationConstructor | undefined,
): WatchNotificationPort {
  const api = source
    ?? (globalThis as { Notification?: BrowserNotificationConstructor }).Notification;
  if (api === undefined) {
    return {
      permission: 'unsupported',
      requestPermission: () => Promise.resolve('unsupported' as const),
      show: () => undefined,
    };
  }
  return {
    get permission(): WatchNotificationPermission {
      return readPermission(api.permission);
    },
    async requestPermission(): Promise<WatchNotificationPermission> {
      const current = readPermission(api.permission);
      // Asking again after an answer is not a second chance: browsers ignore
      // it, and a denied user who is prompted on every Start is being
      // pestered by a surface that was told no.
      if (current !== 'default') return current;
      try {
        return readPermission(await api.requestPermission());
      } catch {
        // A browser that refuses the request outright leaves the page in
        // `default`, which readiness already reports as no fallback.
        return readPermission(api.permission);
      }
    },
    show(title: string, body: string, tag: string): void {
      if (readPermission(api.permission) !== 'granted') return;
      try {
        void new api(title, { body, tag });
      } catch {
        // A failed post is not worth breaking an alert over; the on-page
        // alert and the sound are the primary channels.
      }
    },
  };
}

/**
 * Decides whether a page-level notification is warranted, and remembers what
 * it has already said.
 *
 * The rules are the approved ones, in one place so they can be tested without
 * a browser:
 *
 * - a Section that opens while the page is hidden, or while its audio cannot
 *   actually play, warrants one notification;
 * - an alert the page DID show, whose sound then came back blocked or failed,
 *   warrants one -- this is the ordering hole: the alert arrives while the
 *   page looks healthy, and only the cue outcome says otherwise;
 * - recovery that has been failing for longer than the approved grace period
 *   warrants one, once per outage;
 * - and never more than one per subject.
 */
export const WATCH_RECOVERY_NOTIFICATION_GRACE_MILLISECONDS = 120_000;

export class WatchNotificationLedger {
  readonly #announced = new Set<string>();
  #nextId = 0;
  /** The outage this ledger has already reported, by its first attempt time. */
  #reportedOutageAt: number | null = null;

  /**
   * One request for this subject, or `null` when it has already been made.
   *
   * The key is the SUBJECT, not the kind: an alert that arrives while hidden
   * and whose sound then fails is one thing the user needs to know, not two.
   */
  request(
    kind: WatchNotificationKind,
    subject: string | null,
    sectionKey?: { readonly term: string; readonly campus: string; readonly index: string },
  ): WatchNotificationRequest | null {
    const key = subject ?? `page:${kind}`;
    if (this.#announced.has(key)) return null;
    this.#announced.add(key);
    this.#nextId += 1;
    return {
      id: this.#nextId,
      kind,
      subject,
      ...(sectionKey === undefined ? {} : { sectionKey }),
    };
  }

  /**
   * A request about a recovery that has been failing since `since`, or `null`.
   *
   * Reported once per outage. `clearOutage` is what makes the next one
   * reportable, so a page that reconnects and drops again says so again.
   */
  requestOutage(since: number, now: number, graceMilliseconds: number): WatchNotificationRequest | null {
    if (now - since < graceMilliseconds) return null;
    if (this.#reportedOutageAt === since) return null;
    this.#reportedOutageAt = since;
    this.#nextId += 1;
    return { id: this.#nextId, kind: 'MONITORING_DEGRADED', subject: null };
  }

  clearOutage(): void {
    this.#reportedOutageAt = null;
  }

  /** Forgets an alert entirely, so a later one about it can be posted. */
  forget(subject: string): void {
    this.#announced.delete(subject);
  }
}
