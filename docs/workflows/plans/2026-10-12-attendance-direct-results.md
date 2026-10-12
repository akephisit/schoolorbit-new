# Implementation plan

1. Remove per-person note models, validation, persistence, history and conflict-merge fields with their current domain owners. Preserve calendar notes and correction reasons.
2. Add migration 106 with transactional audit-shape preflight, staged sanitized snapshots, preservation reconciliation and column cleanup. Cover old notes, reset/deleted sessions, refusal and retry in owned database tests.
3. Add typed shared Toggle Group wrappers using the installed Bits UI and existing button tokens. Replace the Attendance result dropdown, remove note controls, and update responsive roster/conflict/history presentations.
4. Regenerate API artifacts; update meaningful runtime/browser fixtures and tests. Run focused database, merge and production-build browser cases, then `./scripts/pipeline verify --scope auto`.
5. Review the frozen diff inline, record evidence and cutover requirements in the PR and canonical Operations/Testing docs. Remove these completed workflow inputs once the PR records the outcome. Merge through the Pipeline gate and track exact-main deployment and maintenance exit.
