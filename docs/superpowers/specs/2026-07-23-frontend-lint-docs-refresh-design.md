# Frontend Lint and Documentation Refresh Design

**Date:** 2026-07-23

## Goal

Restore a trustworthy frontend quality baseline by making the existing lint
command pass, then update current project guidance so developers and AI tools
see the actual 178-operation API checkpoint and the completed reliability,
service-modularity, and timetable-performance work.

## Scope

### Frontend lint stabilization

- Fix all 11 current ESLint errors without disabling rules.
- Replace mutable native `Map` and `Set` instances that participate in Svelte
  reactivity with `SvelteMap` and `SvelteSet`.
- Replace the synchronized `$state` plus `$effect` pattern in
  `ExamInvigilatorPanel.svelte` with writable derived state.
- Remove unsafe control flow from `finally` blocks while preserving loading
  cleanup and stale-request protection.
- Format the 26 files currently reported by Prettier.

No new dependency, route, API, database migration, or user-visible feature is
introduced.

### Current documentation refresh

Update current source-of-truth documentation:

- `.rules`
- `IMPROVEMENT_PLAN.md`
- `docs/TESTING.md`
- `docs/backend-school/API_DEVELOPMENT.md`
- `docs/PROJECT_IMPROVEMENT_ANALYSIS_2026-07-21.md`

The generated contract checkpoint becomes 178 unique operations and includes
`getActivitySlotTimetableContext`. The improvement analysis will retain its
original findings but mark completed readiness, service modularization,
timetable batching, and frontend request-cancellation work as resolved on
2026-07-23.

Historical implementation plans and specifications remain unchanged because
their operation counts and pending items describe the state at the time they
were written.

## Implementation approach

Lint output is the failing baseline and regression gate. Each Svelte fix will
preserve the existing data flow:

- reactive collections keep the same keys, values, and mutation methods;
- writable derived selection continues to allow temporary user overrides and
  recalculates when its source inputs change;
- `finally` blocks perform cleanup only, while stale responses are ignored
  through ordinary conditional flow outside cleanup.

Prettier will apply mechanical formatting only to files in its failing list.
Documentation changes will use the generated OpenAPI artifact and merged
implementation plans as evidence rather than inferred counts.

## Verification

- Run the official Svelte autofixer on every behavior-edited `.svelte` file
  until it reports no issues or suggestions.
- Run `npm run lint`.
- Run `npm run check`.
- Run `npm run test:static`.
- Run `npm run check:api-contracts`.
- Run `npm run test:api-contracts`.
- Run `git diff --check`.
- Confirm generated contract operation count is 178.

## Non-goals

- No Prometheus or Grafana setup.
- No multi-replica realtime backplane.
- No shared Rust contract crate.
- No auto-scheduler restoration.
- No rewrite of historical plans or specifications.
