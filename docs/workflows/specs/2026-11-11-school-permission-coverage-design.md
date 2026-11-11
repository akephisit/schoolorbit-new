# School permission coverage remediation

The user approved fixing all fourteen P1/P2 audit findings. Revalidate the findings against current main before changing their owners. Preserve tenant isolation and existing data; no migration is intended.

Backend domain policies remain authoritative for resource scopes and grant mutations. Extend their typed read projections with resource capabilities where controls depend on a specific resource. Generate OpenAPI and TypeScript artifacts from Rust; frontend consumers must not duplicate organization membership or delegation policies.

The bounded scope covers staff role and membership grants and create-only access, feature management, workflow grant selection, supervision result visibility and scoped actions, organization delegation controls, leave print ownership, daily timetable access, homeroom and student-year directory reads, independent academic year/term management, admission action context, and offering management.

Offering and supervision detail DTOs expose `canManage` and observation `canReview`; organization units expose `can_manage_delegations` and delegation rows `can_revoke`. Academic setup exposes independently readable years/terms; term-only readers receive parent-year context. Shared context-setting transactions continue to require both management grants.

The student-year owner provides `GET /api/academic/student-years/homeroom-options` as complete bounded `HomeroomReference` results, rejecting overflow explicitly. Homeroom directory rows carry grade/program display names. Rich published program choices admit the exact homeroom/student-year management permissions only when those editors open.

Directory reads must succeed under their owning read permission. Protected editor choices load only when the independently authorized action opens; their narrow backend lookups expose only necessary references. Staff creation without grant authority leaves roles and memberships unassigned, with no implicit privileged default. Mutation controls use exact permissions and resource capabilities, and dispose pending action reads when access or identity changes.

Frontend and Backend writers own disjoint source paths. Backend owns generated contracts. The Senior owns Git, contract coordination, candidate freeze and delivery. Independent review and QA assess the complete candidate after integration.

Acceptance requires allowed and denied limited-grant paths, mixed-resource scopes, lazy reads and permission revocation. Focused backend policy/HTTP fixtures and browser workflows cover behavior; contract checks, Svelte/TypeScript checks, focused lint and production build validate integration.
