# Compact – ST-20251113-act-005-03-language-toggle

## Confirmed facts
- Added `LanguageSwitcher` (`frontend/src/components/LanguageSwitcher.tsx` + CSS) that lists every `supportedLocales` entry, pulls localized labels from `common.languageSwitcher.*`, syncs with i18next `languageChanged` events, and persists the chosen locale under `bcsp:locale` in `localStorage` before calling `i18n.changeLanguage`.
- `App` now renders a toolbar hosting the switcher and styles it responsively (`frontend/src/App.tsx`, `frontend/src/App.css`), so the toggle is present on every page load.
- `frontend/src/i18n/index.ts` now exports `supportedLocales`/`LocaleKey`, reads any stored locale at boot, sets `<html lang>` accordingly, listens for subsequent language changes to keep the DOM attribute in sync, and exposes the `localeStorageKey` for reuse.
- `frontend/i18n/messages.json` gained `common.languageSwitcher` entries (labels + per-locale names) in both `en` and `zh`, enabling the UI copy above.
- Created `scripts/i18n_missing_check.ts` plus the `npm run i18n:check` script (package root) to flatten the reference locale (prefers `en`) and fail if any other locale misses a key or changes a value type.
- Self-tests: `npm run build` (frontend) and `npm run i18n:check`.

## Interface / behavior changes
- Users now see a global language toggle; switching languages re-renders immediately, persists the choice, and updates the document `lang` attribute for downstream a11y/formatting logic.
- Contributors must keep `frontend/i18n/messages.json` in sync across locales, because `npm run i18n:check` will now block commits/CI if keys or value types diverge.

## Risks / TODOs
- The translation check script exists but is only exposed as `npm run i18n:check`; wiring it into the actual CI pipeline is still pending if we want automated enforcement.

## Testing
- `npm run build`
- `npm run i18n:check`

## Code Review - ST-20251113-act-005-03-language-toggle - 2025-11-18T20:47:47Z
Codex Review: Didn't find any major issues. Keep them coming!
