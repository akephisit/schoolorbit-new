# PostgreSQL VPS implementation plan

Design: ../specs/2026-10-13-postgresql-vps-design.md.

1. Replace Neon provisioning with ownership-checked PostgreSQL provisioning. Add
   encrypted credentials and public/private DTO separation through forward Admin
   migrations. Use separate runtime, migration and provisioning connections.
2. Remove lazy request migrations. Keep the canonical tenant runner, apply runtime
   grants after migrations, and verify actual schema readiness before serving.
3. Add UUIDv7 defaults and Rust business IDs without changing historical IDs. Use
   OLD/NEW for academic edits, and bound cache/session/log resource use.
4. Update local fixtures, installer, production workflow/configuration and canonical
   rules/docs. Run focused privilege, encryption, retry, isolation, audit, cleanup
   and recovery tests plus ./scripts/pipeline verify --scope auto.
5. Rehearse protected copies, reconcile encoding/collation, counts, checksums,
   identities and migration history. Deliver through PR/Pipeline gate, cut over
   under maintenance, verify every tenant and authenticated proxy/browser flows,
   and restore the actual-data pgBackRest backup before reopening.

Do not edit an applied migration, expose secrets/PII, delete business history,
silently fall back to Neon, or claim an unrun check passed. Remove these workflow
inputs once the implementation PR records the completed outcome.
