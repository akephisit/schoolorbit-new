# Delivery contextual actions

The user authorizes adding basic courses, additional courses and activities beside their delivery tables, removing the generic stop action, and deleting offerings or groups from the selected draft version. Published delivery versions, the catalog, timetable, attendance and results keep their existing identities and behavior.

Delivery remains the provider and persistence owner. Internal `excluded_offering_ids` and `excluded_group_ids` arrays on the change-set record express selected-version exclusions. A forward migration extends the canonical revision snapshot function so refresh and candidate-date preview retain exclusions. Published serialized snapshots are never rewritten. No new crate, permission or parallel graph owner is needed.

Typed DELETE operations under `/term-change-sets/{id}/offerings/{offeringId}` and `/term-change-sets/{id}/groups/{groupId}` accept `changeSetRowVersion` and `targetDeliveryVersionRowVersion` and return the existing change-set DTO. They require authenticated tenant context, existing manage scope for the target, a writable term, matching draft contexts and current revisions. Lifecycle locks precede change-set, version, offering and group locks. Removal clears pending commands for the removed resources and writes its audit atomically. Public wording is `ลบจากรุ่นนี้`; removal never closes the live offering.

Only exclusive unpublished registry resources without other snapshots, commands, student memberships or downstream history may be physically deleted. Shared and published registry resources remain intact and only leave the selected draft graph. Explicit offering inclusion clears that selected exclusion. Publication promotes only groups in its target graph.

Management catalog options expose required-nullable `subjectType` from the pinned subject version's authoritative type, with null for activities. The frontend uses it for three explicit add entries without inferring type from codes or requirement kinds.

Verification covers stale and denied requests, selected-draft isolation, refresh and candidate-date persistence, guarded exclusive cleanup, preserved integrations, pending-command removal, explicit reinclude and publication group membership. Use disposable Delivery fixtures and generated API checks; browser fixtures establish the contextual actions and retained conflict/error behavior.
