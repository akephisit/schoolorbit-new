---
name: schoolorbit-development
description: Coordinate SchoolOrbit implementation and review using repository rules, task-specific agent roles, local verification and the PR delivery flow. Use for SchoolOrbit code, contract, database or pipeline changes and explicit requests for parallel agents. Do not use for unrelated questions or general chat.
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
3. Consult `.rules` → `Agent roles and coordination`. The primary agent acts as Coordinator.
   Use a bounded inline workflow for a one-owner change. When explicitly invoked as
   `$schoolorbit-development`, this skill authorizes the bounded parallel workflow in `.rules`;
   otherwise delegate only if the user has authorized it. Honor inline-only requests.
4. For authorized independent work, delegate exploration, implementation or review using the host's
   available subagent tools. Choose only needed roles and supported model/reasoning controls.
   If subagents or configurable models are unavailable, carry out the same roles inline and say so.
   Do not assume a custom agent configuration file creates agents or is loaded in Codex Cloud.

## Assign bounded tasks

For every child, provide this assignment in the tool message:

```text
Role and outcome:
Repository/worktree and branch:
Base commit and tree:
Allowed paths and read-only or writer status:
Protected/shared files and their designated owner:
Dependencies and agreed interface:
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
2. Freeze the integrated candidate. For authorized parallel work, delegate its read-only
   Rules/Security review and local verification concurrently when independent. Assign one heavy
   Cargo owner and isolated database fixtures under `.rules`.
3. The Verifier follows [testing](../../../docs/TESTING.md) and runs
   `./scripts/pipeline verify --scope auto` plus affected coverage from the verification matrix.
   Report exact commands, tested tree/environment, failures and unrun required checks.
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

Final reporting states what changed, which checks ran, the PR/integration status and material
limitations. Describe model/host capabilities actually used; role instructions are not a security
sandbox, and faster execution or lower cost requires measurement.
