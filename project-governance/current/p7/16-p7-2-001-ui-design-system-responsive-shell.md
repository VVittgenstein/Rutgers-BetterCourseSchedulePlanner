# P7.2-001 — Independent UI design system and responsive shell

- Task: `P7.2-001`
- Parent: `6402157e3b0c23056761bd5e6e8e540b2efa6c6b`
- Branch: `codex/p7-implementation`
- Skills used: `$industrial-brutalist-ui`, `$design-taste-frontend`
- Next task after PostPush: `P7.2-002`

## Design decision

The one shared local/public WebUI uses the **Swiss Industrial Print** archetype.
It does not mix in the dark tactical/CRT archetype. The fixed palette is matte
paper `#efeee8`, carbon ink `#11110e`, and one hazard-red accent `#d42b1e`.
The interface uses square corners, visible grid lines, high-contrast sans display
type, monospace operational data, and no gradients, outer glows, or card shadows.

Desktop uses an asymmetric masthead, navigation band, information rail, status
grid, target index, and primary work plane. Below 768 px it becomes a strict
single-column layout with 44 px controls and no horizontal overflow. Reduced
motion, visible focus, skip navigation, semantic landmarks, and bilingual
language controls are part of the baseline.

## Product result

The shell consumes the real typed runtime, loads FilterSchema and Catalog
discovery concurrently, aborts superseded requests, and renders truthful loading,
valid-empty, error/retry, current, stale, and not-yet-observed lag states. It shows
the real filter/target counts and lets the user choose a published term/campus.
It introduces no fake course data and does not implement P7.2-002 search flows or
P7.3 polish early.

## Verification

- Frontend guard: PASS 80/80
- Vitest component/state/accessibility suite: PASS 37/37
- TypeScript and local/public production builds: PASS
- Public DOM/route/i18n/bundle boundary: PASS 72/72
- Real Chrome snapshots with no horizontal overflow: PASS 5/5
- Viewports: 1440×1000, 1024×768, and 390×844

Snapshots are under `project-governance/current/p7/evidence/p7-2-001/`.
