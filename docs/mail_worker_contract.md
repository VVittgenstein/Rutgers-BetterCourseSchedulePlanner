# Mail notification worker contract

Defines the contract between the `open_events` fan-out producer and the email worker (`mail_dispatcher`). The goal is to make email delivery idempotent, retryable, and forward-compatible while keeping the SQLite queue as the single source of truth.

## Queue surface (SQLite)
- **Source**: `open_event_notifications` rows created by `workers/open_sections_poller.ts` when a section flips `Closed/Wait → Open`.
- **Fields used by the worker**: `notification_id`, `open_event_id`, `subscription_id`, `dedupe_key`, `fanout_status (pending|sent|skipped|failed|expired)`, `fanout_attempts`, `last_attempt_at`, `locked_by`, `locked_at`, `error` (JSON text).
- **Locking**: consumer selects `fanout_status='pending'` with `locked_at IS NULL OR locked_at < now() - lockTtl` (default lock TTL 120s), sets `locked_by` to the worker id and `locked_at=now()`. `fanout_attempts` is incremented exactly once per completed MailSender attempt when persisting the result (sent/retryable/failed), never during lock acquisition.
- **Idempotency**: uniqueness on `(open_event_id, subscription_id)` plus `dedupe_key` copied from `open_events` ensures at most one email per event+subscription per 3m dedupe window. `MailMessage.dedupeKey` must reuse the same value for provider-level dedupe.

## Message payload shape (handed to the email worker)
The worker builds the job payload by joining `open_event_notifications` → `open_events` → `subscriptions` (+`sections`/`courses` for enrichment when available). JSON Schema (Draft 2020-12):

```json
{
  "$id": "https://better-course-schedule/notify/mail_job.schema.json",
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": [
    "version",
    "notificationId",
    "openEventId",
    "subscriptionId",
    "dedupeKey",
    "locale",
    "recipient",
    "template",
    "event",
    "links"
  ],
  "properties": {
    "version": { "type": "string", "enum": ["1.0"] },
    "notificationId": { "type": "integer", "minimum": 1 },
    "openEventId": { "type": "integer", "minimum": 1 },
    "subscriptionId": { "type": "integer", "minimum": 1 },
    "dedupeKey": { "type": "string" },
    "traceId": { "type": "string" },
    "locale": { "type": "string" },
    "recipient": {
      "type": "object",
      "required": ["email"],
      "properties": {
        "email": { "type": "string", "format": "email" },
        "name": { "type": "string" }
      },
      "additionalProperties": false
    },
    "template": {
      "type": "object",
      "required": ["id", "variables"],
      "properties": {
        "id": { "type": "string", "const": "open-seat" },
        "version": { "type": "string", "default": "v1" },
        "variables": {
          "type": "object",
          "required": ["courseTitle", "courseString", "sectionIndex", "sectionNumber", "meetingSummary", "manageUrl"],
          "properties": {
            "courseTitle": { "type": "string" },
            "courseString": { "type": "string" },
            "sectionIndex": { "type": "string" },
            "sectionNumber": { "type": "string" },
            "meetingSummary": { "type": "string" },
            "campus": { "type": "string" },
            "eventDetectedAt": { "type": "string", "format": "date-time" },
            "manageUrl": { "type": "string", "format": "uri" },
            "unsubscribeUrl": { "type": "string", "format": "uri" },
            "notes": { "type": "string" }
          },
          "additionalProperties": true
        }
      },
      "additionalProperties": false
    },
    "event": {
      "type": "object",
      "required": ["termId", "campusCode", "indexNumber", "statusAfter", "eventAt"],
      "properties": {
        "sectionId": { "type": "integer" },
        "termId": { "type": "string" },
        "campusCode": { "type": "string" },
        "indexNumber": { "type": "string" },
        "sectionNumber": { "type": "string" },
        "subjectCode": { "type": "string" },
        "courseTitle": { "type": "string" },
        "statusBefore": { "type": "string" },
        "statusAfter": { "type": "string" },
        "eventAt": { "type": "string", "format": "date-time" },
        "detectedBy": { "type": "string" },
        "snapshotHash": { "type": "string" }
      },
      "additionalProperties": true
    },
    "subscription": {
      "type": "object",
      "required": ["status", "contactType"],
      "properties": {
        "status": { "type": "string", "enum": ["pending", "active", "paused", "suppressed", "unsubscribed"] },
        "contactType": { "type": "string", "const": "email" },
        "contactValue": { "type": "string", "format": "email" },
        "contactHash": { "type": "string" },
        "lastKnownSectionStatus": { "type": "string" },
        "preferences": {
          "type": "object",
          "properties": {
            "notifyOn": { "type": "array", "items": { "type": "string" } },
            "maxNotifications": { "type": "integer" },
            "deliveryWindow": {
              "type": "object",
              "properties": {
                "startMinutes": { "type": "integer" },
                "endMinutes": { "type": "integer" }
              }
            },
            "snoozeUntil": { "type": "string", "format": "date-time" }
          },
          "additionalProperties": true
        }
      },
      "additionalProperties": true
    },
    "links": {
      "type": "object",
      "properties": {
        "manageUrl": { "type": "string", "format": "uri" },
        "unsubscribeUrl": { "type": "string", "format": "uri" }
      },
      "additionalProperties": false
    },
    "deliveryPolicy": {
      "type": "object",
      "properties": {
        "maxAttempts": { "type": "integer", "minimum": 1, "default": 3 },
        "lockTtlSeconds": { "type": "integer", "minimum": 30, "default": 120 },
        "retryScheduleMs": { "type": "array", "items": { "type": "integer", "minimum": 0 }, "default": [0, 2000, 7000] }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}
```

Notes:
- `event.termId/campusCode/indexNumber/sectionId` come from `open_events`; `snapshotHash`, `courseTitle`, and `sectionNumber` are stored in `open_events.payload` at detection time to avoid stale joins.
- `subscription` must be filtered to `contact_type='email'`; other channel types are skipped with a terminal status.
- `template.variables` is intentionally open for additive, locale-specific fields; the listed keys are the minimum required by v1 templates.

## Ack/Nack and retry semantics
Authoritative rule: `fanout_attempts` increments exactly once per MailSender attempt (sent/retryable/failed). Any prior failure-only or pre-send increment semantics (including earlier drafts or compacts) are superseded and should be ignored.
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, bump `fanout_attempts` by 1, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, bump `fanout_attempts` by 1, keep `fanout_status='pending'`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[fanout_attempts-1]` (after increment) capped by `lockTtlSeconds`.
- **Terminal nack**: for `status="failed"` (invalid recipient, template missing locale/variable, unsubscribed row, non-email contact), bump `fanout_attempts` by 1, set `fanout_status='skipped'` for non-actionable rows or `fanout_status='failed'` when the channel should alert ops. Record `{code,message}` in `error` and clear the lock.
- **Dead-letter**: when `fanout_attempts >= deliveryPolicy.maxAttempts`, move the row to `fanout_status='failed'` regardless of the MailSender result and preserve the last attempt in `error`. Operators can requeue by resetting `fanout_status='pending'`, `fanout_attempts=0`, and `error=NULL`.
- **Lock expiry**: if a worker crashes mid-attempt, another worker may take over after `lockTtlSeconds`; duplicate sends are prevented by `dedupe_key` and provider-level `dedupeKey`.

## MailSender invocation expectations
- The worker must invoke `ReliableMailSender` (or a `MailSender` adapter) with:
  - `to.email` from `subscription.contact_value`; `locale` derived from subscription or deployment default.
  - `templateId='open-seat'`, `templateVersion` from the job payload, `variables` from `template.variables`, `unsubscribeUrl`/`manageUrl` from `links`.
  - `dedupeKey` from `open_event_notifications.dedupe_key`; `traceId` from `open_events.trace_id`.
  - `metadata` carries `{ subscriptionId, openEventId, term: event.termId, campus: event.campusCode }`.
- `SendResult` must not be thrown; structured outcomes determine the ack/nack path above. Provider retries are handled inside `ReliableMailSender`; `fanout_attempts` only tracks outer queue tries (incremented once per MailSender attempt).

## Versioning
- `version` uses `MAJOR.MINOR`. Minor releases are additive (new optional fields only); bump the major when removing/renaming fields or changing semantics. Producers must emit the lowest supported major until all workers are upgraded.
- `template.version` allows swapping email copy/layout without changing queue semantics; workers pass it through to template loaders.
- Store only additive data in `error` JSON to remain parseable by older tooling; never change the meaning of `fanout_status` values without a major bump.
