# Backend HTTP boundary implementation

1. Refresh main, create the dedicated branch, capture baseline and inspect callers.
2. Extract navigation and notification models/services/events; preserve root facades.
3. Centralize authentication HTTP context with no dependency on full AppState.
4. Extract certificate handlers and domain paths using narrow FromRef state and
   a file-deletion port; preserve public verification/client-address behavior.
5. Extract academic handlers/domain paths and inject existing lifecycle/result-lock
   ports plus a typed realtime port. Keep application-owned adapters and routing.
6. Update source guards to inspect actual owners and add state/boundary coverage.
7. Run cargo fmt, changed-package tests, static_architecture, workspace all-target
   checking, API artifact equality and permission contracts; run relevant disposable
   DB fixtures, frontend lint/type/static checks and auth/realtime acceptance.
8. Run paired same-runner Docker before/after builds, two samples per case, retaining
   initial graph priming separately and preserving production compiler flags.
9. Record evidence in PR, update canonical ownership/rules, remove these temporary
   workflow inputs, refresh main and integrate only the verified candidate.
10. Verify all main CI and production School release smoke/acceptance gates.
