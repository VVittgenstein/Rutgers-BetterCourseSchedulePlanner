# Public Web Target

Status: accepted product target for planning  
Date: 2026-07-11  
Scope: Preflight 0A for dual delivery

This document records the target for **B: the public web deployment**. It is
not an NGAT handoff and does not approve implementation.

## Purpose

B is the public web version of Rutgers BetterCourseSchedulePlanner. Its purpose
is to let users who do not know GitHub, cannot install the local package, or are
not on Windows open a website directly and use the service.

The ordinary-user experience should match **A: the Windows local release
package**. A and B should share the same WebUI and the same core course-planning
experience wherever the deployment context allows it.

## Hard Product Boundaries

- A and B do not include email reminders.
- A and B do not include SendGrid, SMTP, mail config UI, or mail-worker delivery
  as part of the current release surface.
- Email reminders should be marked as future work on GitHub, not presented as a
  current feature.
- B does not expose local/admin configuration surfaces to ordinary public users.
- B should be usable from both desktop and mobile browsers.

## Shared User Experience

A and B should provide the same ordinary-user WebUI for:

- course search;
- course filtering;
- course and section inspection;
- section open/closed status;
- section subscription;
- browser audio alerts;
- language and interaction behavior, subject to later UI design decisions.

Detailed UI differences between A and B should be decided during the UI design
phase. This document only fixes the product behavior.

## Subscription Model

Subscriptions are active browser-session watches, not offline or persistent
server-side reminders.

- A user can subscribe to sections from the open WebUI.
- A single browser session can actively subscribe to at most **9 sections**.
- Active subscription state exists only while the WebUI session is active.
- The server keeps live watch state in memory for active connections.
- Closing the tab, closing the browser, disconnecting, locking a phone, or the
  browser being suspended means audio alerting is not guaranteed.
- The browser may remember user selections locally for convenience, but active
  watching must be started by the user in an open WebUI session.

Subscriptions should be keyed by section. NGAT should verify whether Rutgers
section/index numbers are unique enough for the target scope. If not, the
subscription key must include enough context, such as `term + campus + section
index`.

## Alert Semantics

The alert trigger is based on received Open status messages, not only on a
Closed-to-Open transition.

- If a subscribed section is Open, every server message that reports it as Open
  should trigger browser audio while the subscription is active.
- No debounce is required.
- The WebUI must provide a volume control.
- The WebUI must allow users to choose one-shot audio or continuous audio.
- There is no email fallback in the current release.

## Data Refresh Targets

Course-directory data and open-seat status have different refresh semantics.

| Data | A: Windows local package | B: Public web deployment |
|---|---|---|
| Course directory | Default refresh interval: 10 minutes. User/config can change it. | Default refresh interval: 10 minutes. Ordinary users cannot configure it. |
| Open-seat status | Target from status update to browser alert: under 1 second where feasible. | Target from status update to browser alert: under 1 second where feasible; must be validated against hosting and network limits. |

The 1-second target is an aspirational product target for active WebUI sessions.
Actual B performance must be validated during platform selection and later load
testing.

## Public Web Runtime Model

B should use a server-centralized status model:

1. The B server maintains the shared course and section database.
2. The B server refreshes course-directory data on its configured schedule.
3. The B server centrally polls Rutgers openSections for open-seat status.
4. Browsers do not directly poll Rutgers openSections.
5. When a user starts watching sections, the browser opens a WebSocket
   connection to the B server. The exact message protocol and reconnect policy
   remain design work.
6. The server tracks which live browser connections are watching which sections.
7. When watched section status is sent as Open, the browser plays audio according
   to the user's selected audio mode and volume.

This centralizes Rutgers API traffic, avoids one Rutgers poller per browser, and
lets the server fan out small status messages to active WebUI sessions.

## Open Questions for Later Phases

- Exact WebSocket message schema, heartbeat, lag, and reconnect policy.
- Exact openSections polling cadence that can meet the 1-second target without
  violating upstream rate limits.
- Load-test acceptance thresholds for the selected Vultr host.
- Final UI treatment for A/B-only configuration differences.
