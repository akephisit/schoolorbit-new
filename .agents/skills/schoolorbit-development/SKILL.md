---
name: schoolorbit-development
description: Automatically use for ordinary SchoolOrbit implementation, bug fixes, refactoring, tests, code reviews, contracts, database, pipeline, development-rule or skill changes; explicit invocation is optional. Run a Senior-led team with Frontend, Backend, independent Review and QA roles and repository model/reasoning profiles. Keep small or tightly coupled tasks inline and honor inline-only requests. Do not turn general questions into implementation or use for unrelated chat.
---

# SchoolOrbit development workflow

Read the repository-root [`.rules`](../../../.rules) first. It is the sole development standard.
Use the [documentation index](../../../docs/README.md) to select relevant references.
Read `.rules` from the assigned checkout after every resume or base update.
This skill routes work; domain conventions and agent policy stay in `.rules`.
Apply `.rules` → `Automatic workflow activation` for ordinary project work requests without a
special command. A delegated agent keeps its assigned role; loading this file does not create
another Coordinator. Explanation-only requests do not start implementation or PR delivery.

## Start and choose roles

1. Establish the user's requested outcome and current authorization, inspect the actual implementation,
   and refresh GitHub as required by `.rules`. Preserve work owned by other sessions.
2. State a short plan, impacted owners, dependencies and applicable verification before editing.
   Apply `.rules` → `Applicability and conflicting examples` and record the working agreement
   from `Consistency decisions and final review`; retain it through implementation and resume.
   Use the approved specification/plan process for architectural work.
3. Consult `.rules` → `Agent roles and coordination`. The primary agent acts as Senior/Coordinator:
   settle the technical direction, contracts and acceptance criteria, then assign work by role.
   Automatic workflow activation applies the project's standing delegation preference;
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
3. Spawn only ready tasks with the assignment below. Use one feature branch/shared checkout by
   default and assign disjoint files before parallel writers start. Keep shared files, generators
   and Git mutations with the designated owner under `.rules`. Use a separate worktree only for
   the concrete isolation cases defined there, not merely because Frontend and Backend run together.
4. Advance independent Coordinator work while children run. Follow their results at dependency
   boundaries, resolve overlaps or blockers, and start dependent work only after inspecting the
   prerequisite output. Keep spawning and reassignment with the Coordinator.
5. Once every writer finishes and the Senior reconciles the changes, freeze the complete candidate
   and switch to the independent Reviewer and QA/Verifier phase. Keep the shared source unchanged.
   The Reviewer checks correctness and rules; QA runs the selected checks. Return defects to the
   owning writer, then repeat affected review/checks. The Senior accepts the result only after
   reconciling both reports. Keep the role roster, actual settings and handoffs in task state.

## Open a draft and save checkpoints

1. Follow `.rules` → `Git branches, worktrees, and integration`: after the first meaningful commit,
   push and open/attach a draft PR within the user's delivery scope. Reuse that PR for the feature;
   do not wait for final implementation or all checks to finish. Honor local-only/no-push requests.
2. At a useful milestone or handoff, the Senior pauses all writers and source-mutating tools and
   waits for acknowledgement. Inspect and stage only the agreed checkpoint paths, preserve other
   unfinished work, review the staged diff and run `git diff --cached --check`, then commit/push.
3. Record the checkpoint HEAD, remaining work and actual check status in task/PR state, notify
   agents of the new HEAD, and resume their assignments. Do not repeat the full matrix solely to
   save a checkpoint or describe an untested checkpoint as verified.
4. Keep the PR draft while work, independent review or required checks are incomplete. Mark it
   ready only after the final acceptance process below and the applicable checks pass; readiness,
   merge and production actions remain bounded by the user's scope and `.rules`.

## Assign bounded tasks

For every child, provide this assignment in the tool message:

```text
Role and outcome:
Requested model/reasoning profile and host-confirmed settings or fallback:
Repository/shared checkout and branch, or isolated worktree with its reason:
Base commit and tree:
Allowed paths and read-only or writer status:
Protected/shared files and their designated owner:
Shared Git, generator, formatter and Rust build ownership:
Dependencies and agreed interface:
Completion criteria and blocking conditions:
Applicable .rules sections and canonical references to read:
Focused verification and fixture ownership:
Deliverable: owned changed paths, evidence, findings and remaining uncertainty;
commits only for an assigned isolated worktree, candidate tree supplied by the Senior.
In a shared checkout, do not stage/commit, switch branches, stash/reset/clean or revert others' work.
Acknowledge the Senior's checkpoint pause; resume only after the Senior confirms the new HEAD.
No independent PR readiness, main changes or production operations.
```

Start writer work only after the Coordinator establishes checkout mode and non-overlapping file
ownership. Shared-checkout writers hand back edits in place; the Senior owns the shared Git index.
Prefer fresh, bounded task context over copying the whole conversation. Include the current user
decisions explicitly, especially API, migration, maintenance and integration constraints.
When a child discovers overlap or an interface change, stop that dependent work, report it to the
Coordinator and obtain a revised assignment. Independent work can continue.

## Integrate and verify

1. Inspect each deliverable and reconcile shared-checkout edits in dependency order on the feature
   branch; integrate child commits only when isolated worktrees were necessary. The designated
   owner reconciles shared files and generated artifacts. A child test result covers only its
   tested inputs and environment.
2. Freeze the integrated candidate. For parallel work, delegate its read-only
   Rules/Security review and local verification concurrently when independent. Assign one heavy
   Cargo owner, reuse the compatible target sequentially, and use isolated database fixtures under
   `.rules`. No per-agent Rust target or dependency copy is needed for the shared-checkout team.
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
Create the draft early, keep progress in checkpoint commits, and record review and actual local
checks in that same PR; leave incomplete work in draft. Follow
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
