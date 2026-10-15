# School organization and learning areas

Approved scope: the two-tab organization design accepted in chat on 2026-10-15.

- `/staff/organization` owns the administrative tree and a separate learning-area registry.
  Tab and selected unit/group/detail belong to the URL. The academic menu redirects to
  the same learning-area tab; it has no second editor.
- Ordinary learning areas are children of SCHOOL, never administrative groups or divisions.
  AC/ACAD-ACT retains the existing learner-development activity identity and affiliation.
- `subject_groups` owns learning-area names, status and order. Its database trigger maintains
  the one-to-one organization row in the same transaction. Staff retains memberships/grants;
  Academic retains catalog affiliations. This small persisted invariant does not warrant a new
  cross-domain crate or a dependency from Academic Core to Staff HTTP.
- Create requests omit codes; the database allocates MG/DV/SG/OU codes with transactional,
  per-namespace counters. Existing codes and identities remain unchanged.
- Exact school catalog readers/managers can enter the learning-area tab without roles access.
  Role assignment and permission editing keep their separate existing capabilities.
  Unit/tree catalog management alone cannot change the school-level registry.
- Detaching areas changes organization-tree scope intentionally. Preserve all grants in place;
  never copy or broaden grants to compensate. Membership and catalog owner IDs do not change.
- Migration preflight rejects ambiguous mappings, inconsistent status, missing roots,
  children below learning areas and inconsistent subject owners. Only an entirely unlinked
  registry entry with no subject-owner conflict receives a newly generated owner.
- New names win over the organization display projection; English names may be empty.
  Deactivation retains history and refuses live subjects or the activity registry.
- Rollout: universal maintenance, forward migration/reconciliation, both backend/frontend
  consumers, then authenticated acceptance. Old binaries are prohibited after migration 116;
  recovery is roll-forward. No runtime dual-write or legacy fallback is retained.
