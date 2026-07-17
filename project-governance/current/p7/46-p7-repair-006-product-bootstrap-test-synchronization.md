# P7-REPAIR-006 - product bootstrap test synchronization

The full frontend gate exposed one test-only race: the test asserted the
runtime READY marker before React had always run the child `onReady` effect.

The repair waits for that already-required callback before making the existing
READY assertions. It changes no component, product behavior, assertion,
dependency or build configuration.

Acceptance: the focused test passes five consecutive runs; standard
`npm run verify` passes guards 82/82, Vitest 105/105, TypeScript checking,
Local/Public production builds and both target-surface gates.

Next task: source refreeze and replacement Windows/Linux candidate build.
