# Attendance sparse results implementation

Base: origin/main 4453829a25451b15c15921c53499085a8afbb725. Coordinator owns the root feature branch,
all shared contracts and migration numbering. Execute inline; user authorization includes integration
and the Attendance-only reset of snwsb after verification, with no repeat approval checkpoint.

1. Expand immutable session rosters and module generation, backfill/reconcile existing data and
   canonicalize historical ceremonies, then constrain persisted facts to explicit results.
2. Replace mutation-on-open with read-only preview; atomically create sessions on first save,
   enforce roster token/revision/permissions, and patch explicit changed results only.
3. Delay special result creation; make kiosk preparation read-only and create only accepted scans.
   Keep duplicate event and gallery validation, manual priority and File Platform cleanup.
4. Batch dated academic expectations, join sparse committed facts, update report/history/purge and
   digest owners, and remove background snapshot materialization.
5. Add scoped impact/reset operations and generation invalidation, reusing File Platform deletion.
6. Regenerate OpenAPI; update staff/family components and meaningful PostgreSQL/browser fixtures.
7. Run focused suites and the owned aggregate matrix, inspect the complete diff and query plans,
   publish PR, follow integration/deployment, then reset only snwsb and verify cleanup/isolation.

Commands and operational procedures belong to docs/TESTING.md and docs/OPERATIONS.md. Remove these
workflow artifacts after the implementation PR records their completed outcome.
