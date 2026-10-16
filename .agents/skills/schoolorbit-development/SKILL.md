---
name: schoolorbit-development
description: Run SchoolOrbit work as a role-based team led by a Senior/Coordinator, assigning Frontend, Backend, independent Review and QA roles with model and reasoning profiles from repository rules. Use for SchoolOrbit implementation, reviews, contracts, database, pipeline, development-rule or skill changes. Keep small or tightly coupled tasks inline and honor inline-only requests. Do not use for unrelated questions or general chat.
---

# SchoolOrbit development workflow

Read the repository-root [`.rules`](../../../.rules) first. It is the sole development standard.
Use the [documentation index](../../../docs/README.md) to select relevant references.
Read `.rules` from the assigned checkout after every resume or base update.
This skill routes work; domain conventions and agent policy stay in `.rules`.

## Start and choose roles

1. Establish the user's requested outcome and current authorization, inspect the actual implementation,
   and refresh GitHub as required by `.rules`. Preserve work owned by other sessions.
2. State a short plan, impacted owners, dependencies and applicable verification before editing.
   Apply `.rules` → `Applicability and conflicting examples` and record the working agreement
   from `Consistency decisions and final review`; retain it through implementation and resume.
   Use the approved specification/plan process for architectural work.
3. Consult `.rules` → `Agent roles and coordination`. The primary agent acts as Senior/Coordinator:
   settle the technical direction, contracts and acceptance criteria, then assign work by role.
   Explicit or implicit skill selection applies the project's standing delegation preference;
   choose parallel or inline work under those rules without requesting permission for each child.
   Preserve the user's scope, including read-only and inline-only requests.
4. Select each needed role's model/reasoning profile from `.rules` and pass it through the host's
   actual agent-spawn controls. Use fresh bounded context when model overrides require it, and
   record requested settings and any confirmed settings or fallback. Retain the primary session's
   model for the Senior; this skill cannot change it. If subagents are unavailable,
   carry out the roles inline and report that limitation once.
   Do not assume a custom agent configuration file creates agents or is loaded in Codex Cloud.

## Split and schedule work

1. Identify independently useful outcomes and their completion criteria, then record each task's
   owner, allowed paths, prerequisites and status in the existing plan/chat state. Keep a small
   one-owner change inline; shared-file work or unresolved contracts require an ordered assignment.
2. Choose the smallest useful role roster under `.rules`. For a full-stack task, the Senior agrees
   the wire contract and assigns Backend and Frontend writers to their respective owners. Each
   writer implements its layer and focused tests; the Senior resolves shared decisions and
   integrates their work. Open optional Explorer, Data/Migration or Pipeline roles only as needed.
   Read-only requests assign only read-only work; do not create writers for an instruction review.
3. Spawn only ready tasks with the assignment below. Establish separate worktrees before parallel
   writers start. Assign every shared file and generated artifact to one owner.
4. Advance independent Coordinator work while children run. Follow their results at dependency
   boundaries, resolve overlaps or blockers, and start dependent work only after inspecting the
   prerequisite output. Keep spawning and reassignment with the Coordinator.
5. Once implementation is integrated, switch to the independent Reviewer and QA/Verifier phase.
   The Reviewer checks correctness and rules; QA runs the selected checks. Return defects to the
   owning writer, then repeat affected review/checks. The Senior accepts the result only after
   reconciling both reports. Keep the role roster, actual settings and handoffs in task state.

## Assign bounded tasks

For every child, provide this assignment in the tool message:

```text
Role and outcome:
Requested model/reasoning profile and host-confirmed settings or fallback:
Repository/worktree and branch:
Base commit and tree:
Allowed paths and read-only or writer status:
Protected/shared files and their designated owner:
Dependencies and agreed interface:
Completion criteria and blocking conditions:
Applicable .rules sections and canonical references to read:
Focused verification and fixture ownership:
Deliverable: commits/tree, changed files, evidence, findings and remaining uncertainty.
No independent PR readiness, main changes or production operations.
```

Start writer work only after the Coordinator establishes a distinct branch/worktree and file owner.
Prefer fresh, bounded task context over copying the whole conversation. Include the current user
decisions explicitly, especially API, migration, maintenance and integration constraints.
When a child discovers overlap or an interface change, stop that dependent work, report it to the
Coordinator and obtain a revised assignment. Independent work can continue.

## Integrate and verify

1. Inspect each deliverable and integrate in dependency order on the Coordinator's branch.
   The designated owner reconciles shared files and generated artifacts. A child test result covers
   only its tested tree and environment.
2. Freeze the integrated candidate. For parallel work, delegate its read-only
   Rules/Security review and local verification concurrently when independent. Assign one heavy
   Cargo owner and isolated database fixtures under `.rules`.
3. The Verifier selects checks under `.rules` section 11 and uses the matching owning runners or
   focused recipes in [testing](../../../docs/TESTING.md). Record the affected scope and rationale;
   automatic application-wide verification is not a prerequisite for every edit. Reuse valid
   scoped evidence under section 1. Report exact commands, tested tree/environment, failures,
   cancelled checks and unrun required checks.
4. The Reviewer reads current `.rules` and inspects the complete integrated diff for scope, ownership,
   API/permission contracts, tenant isolation, data preservation and operational invariants.
   Check the working agreement and justified exceptions under `Consistency decisions and final review`.
   Report actionable findings with file/line evidence, severity and required correction.
5. Return failures to the assigned writer. After any source/base/environment change, apply the
   evidence invalidation rules in `.rules`; repeat affected checks/review on the new candidate.

## Deliver and resume

Only the Coordinator performs the PR/integration and production workflow defined in `.rules`.
Record review and actual local checks in the PR; leave incomplete work in draft. Follow
[operations](../../../docs/OPERATIONS.md) for selected production changes and acceptance.
Track CI, integration and required acceptance to completion before claiming delivery.

At a handoff or compaction, retain a short task-state checkpoint in chat/tool state:
requested outcome, current user decisions, base/candidate tree, role/branch/path ownership,
completed evidence, unresolved findings and the next dependent action. Re-read actual repository
state when resuming. Do not create another Markdown status report or treat old evidence as current.

Final reporting states what changed, which role did what, requested and confirmed model/reasoning
settings or fallbacks, which checks ran, the PR/integration status and material limitations.
Describe model/host capabilities actually used; role instructions are not a security
sandbox, and faster execution or lower cost requires measurement.
