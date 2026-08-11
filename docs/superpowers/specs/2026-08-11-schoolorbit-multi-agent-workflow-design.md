# SchoolOrbit Multi-Agent Development Workflow Design

## Status

Approved in conversation on 2026-08-11.

## Problem

SchoolOrbit has durable development rules spanning Rust, SvelteKit, PostgreSQL migrations,
generated permission and API contracts, realtime identity, deployment, and PDPA-sensitive data.
A single general-purpose agent can perform this work, but large changes mix planning, codebase
exploration, implementation output, test logs, and review findings in one context. That increases
context noise and makes it easier to miss cross-stack impact or violate a high-risk invariant.

The desired workflow resembles a controlled engineering project: the user owns the outcome, a
planner prepares the design, specialized workers execute approved tasks, and an independent
reviewer and verifier inspect the integrated result. Agents may work concurrently where their
work is independent, but no implementation begins before the user has discussed and approved the
plan.

## Goals

- Make the workflow apply automatically to every SchoolOrbit build, change, fix, refactor, or
  other repository mutation request.
- Require a read-only planning conversation and explicit user approval before implementation.
- Pin planning and review to `gpt-5.6-sol` with `max` reasoning effort.
- Use elevated GPT-5.6 profiles for exploration, implementation, and verification.
- Run independent exploration and implementation lanes concurrently without shared-worktree,
  file-ownership, Git-index, or generated-artifact conflicts.
- Compose existing Superpowers workflows for brainstorming, planning, debugging, TDD, worktree
  isolation, review, and verification instead of copying their instructions.
- Enforce SchoolOrbit's `.rules`, including immutable applied migrations, generated contracts,
  authorization completeness, verification requirements, and PDPA safeguards.
- Test the skill itself with baseline and forward scenarios before treating it as deployed.

## Non-goals

- Do not allow an agent to implement a change merely because it produced a plan.
- Do not make answer, explanation, diagnosis, or read-only review requests authorize file edits.
- Do not run multiple writers in the same worktree or let agents commit concurrently through one
  Git index.
- Do not fork or modify the installed Superpowers plugin.
- Do not replace `.rules`, canonical documentation, generated contracts, or application tests with
  agent instructions.
- Do not make deployment, push, pull-request creation, destructive actions, or external writes
  implicit consequences of plan approval.
- Do not create a permanent implementation diary or additional canonical Markdown documentation.

## Decision

Add a repository-scoped `schoolorbit-workflow` skill that acts as the controller for a hybrid
multi-agent workflow. It uses Superpowers for established process disciplines, while a small
SchoolOrbit-specific layer owns model routing, the approval state machine, the task dependency
graph, protected-resource classification, parallel worktree lanes, and integration barriers.

The skill is implicitly discoverable for every SchoolOrbit mutation request. It does not trigger
implementation for read-only requests. Its description expresses only the trigger condition; the
complete workflow lives in the skill body so Codex must load and follow it.

Implicit matching is not the only enforcement mechanism. Update `.rules` so its required analysis
workflow explicitly invokes `schoolorbit-workflow` before every build, change, fix, refactor, or
other mutation. `AGENTS.md` continues to point to `.rules` as the single authoritative development
standard, avoiding a second competing rule set.

## Mental Model

- The user is the project owner and approves the plan.
- The controller is the site manager that routes work and maintains the workflow state.
- The planner is the lead engineer that prepares the design and work breakdown.
- Explorers are survey teams that collect bounded evidence without changing the site.
- Implementers are specialized trade teams working in isolated areas.
- The reviewer is the independent inspector.
- The verifier performs commissioning tests against the approved requirements.
- Superpowers provides standard operating procedures; `.rules` is the binding project safety and
  engineering code.

## Repository Layout

The implementation adds these repository-owned artifacts:

```text
.agents/skills/schoolorbit-workflow/
  SKILL.md
  agents/openai.yaml
  scripts/validate-work-graph.mjs
.codex/
  config.toml
  agents/
    schoolorbit-planner.toml
    schoolorbit-explorer.toml
    schoolorbit-implementer.toml
    schoolorbit-high-risk-implementer.toml
    schoolorbit-reviewer.toml
    schoolorbit-verifier.toml
frontend-school/tests/static/
  schoolorbit-agent-workflow.test.mjs
```

Update `.rules` and `frontend-school/tests/static/documentation-policy.test.mjs` in the same
change. The Markdown policy permits the exact repository skill entry point and its directly owned
skill Markdown shape without opening a general-purpose documentation directory. The skill starts
self-contained; it does not add Markdown references or auxiliary README files.

## Codex Configuration

Project configuration explicitly enables multi-agent support even though current Codex releases
enable it by default. It sets both the stable feature flag used by Superpowers and the current
agent settings, caps concurrent spawned threads at three excluding the primary controller, and
registers every custom role with a relative TOML config path.

Each custom agent file defines `name`, `description`, `developer_instructions`, `model`,
`model_reasoning_effort`, and `sandbox_mode`. Read-only roles use `read-only`. Implementation roles
write only inside the isolated worktree assigned in their task brief. Live permission and sandbox
overrides continue to be owned by the parent turn.

## Agent Profiles

| Role | Model | Effort | Primary contract |
|---|---|---|---|
| Planner | `gpt-5.6-sol` | `max` | Produce impact analysis, an approval-ready plan, task dependencies, ownership, risk, and verification commands; never edit. |
| Explorer | `gpt-5.6-terra` | `xhigh` | Trace one bounded code or documentation domain and return evidence with file references; never edit. |
| Implementer | `gpt-5.6-sol` | `xhigh` | Complete one approved task in its assigned worktree using TDD and return commits plus test evidence. |
| High-risk implementer | `gpt-5.6-sol` | `max` | Implement approved work involving migrations, authentication, permissions, API contracts, PDPA, realtime identity, or deployment. |
| Reviewer | `gpt-5.6-sol` | `max` | Check requirement compliance, correctness, security, regressions, and test gaps; never edit. |
| Verifier | `gpt-5.6-terra` | `high` | Run the applicable focused and `.rules` verification commands and report fresh evidence; never modify source to obtain a pass. |

Agent dispatches specify the role, model, and effort explicitly. If a client cannot select a
custom role by name, the controller passes the same role instructions and explicit model settings
in the spawn request rather than silently inheriting the primary model.

## Superpowers Composition

The controller invokes the minimum applicable Superpowers skills by name:

- `superpowers:brainstorming` for creative or behavior-changing design discussions, subject to the
  user-approved artifact policy below;
- `superpowers:writing-plans` for multi-file or high-risk implementation plans;
- `superpowers:dispatching-parallel-agents` for independent read-only investigations;
- `superpowers:systematic-debugging` for bugs, unexpected behavior, or failed verification;
- `superpowers:using-git-worktrees` before implementation and for isolated parallel lanes;
- `superpowers:test-driven-development` for implementation and bug fixes;
- `superpowers:requesting-code-review` for task and whole-change review;
- `superpowers:verification-before-completion` before any completion claim;
- `superpowers:finishing-a-development-branch` only after implementation and verification are
  complete and the user is choosing how to integrate the branch.

The controller does not invoke `superpowers:subagent-driven-development` for the implementation
phase because that workflow explicitly serializes all implementers. The SchoolOrbit controller
preserves its bounded briefs, evidence-based review, fix-loop, and final-review principles while
adding isolated parallel worktree lanes.

For small changes, the user-approved SchoolOrbit policy overrides Superpowers' default artifact
requirement: the plan is discussed and approved in chat, but no plan/spec file is created. For a
multi-file or high-risk change, the full Superpowers spec and implementation-plan artifact gates
apply.

## Workflow State Machine

The controller maintains these states:

1. `DISCOVER`: read `.rules`, locate the relevant canonical documentation, inspect the current
   implementation, and dispatch bounded read-only explorers when domains are independent.
2. `DRAFT_PLAN`: the planner produces scope, assumptions, impact analysis, task dependencies,
   owned paths, protected resources, agent profiles, and verification commands.
3. `AWAIT_APPROVAL`: present the plan in chat. No source, configuration, documentation, or test
   write is allowed while waiting.
4. `RECORD_PLAN`: after chat approval, write and self-review a dated spec and plan only for
   multi-file or high-risk work. Ask the user to review the written artifact before execution.
5. `EXECUTE_WAVES`: validate the work graph, create isolated lane worktrees, and dispatch up to
   three dependency-ready implementers concurrently.
6. `INTEGRATE`: integrate successful lane commits onto the integration branch in dependency order,
   one lane at a time.
7. `REVIEW_FIX`: run task and whole-change reviews, route blocking findings back to the owning
   lane, and re-review every fix.
8. `VERIFY`: run fresh focused tests and every applicable command from `.rules`.
9. `COMPLETE` or `BLOCKED`: report verified evidence or the exact unresolved decision. Clean up
   temporary worktrees only after the final review is accepted.

Unambiguous approval includes statements such as "do it", "approve the plan", or an equivalent
clear instruction to proceed. A question, correction, tentative acknowledgment, or approval of
only one design section does not authorize implementation. Any material scope, architecture,
ownership, risk, or verification change invalidates the earlier approval and returns the workflow
to `DRAFT_PLAN`.

## Plan Artifact Policy

Every mutation request receives a concise plan in chat and waits for approval.

- A small, single-purpose, low-risk change proceeds after that one approval without adding a plan
  file.
- A multi-file or high-risk change uses a dated design spec under `docs/superpowers/specs/` and an
  implementation plan under `docs/superpowers/plans/` after chat approval. The controller
  self-reviews the artifact and obtains a second user review before implementation.
- Workflow artifacts remain temporary inputs and are removed after the implementation pull
  request records the completed outcome, as required by `.rules`.

## Work Graph and Ownership

Before implementation, the planner provides a machine-checkable work graph. Each task contains:

```text
id
dependencies
owned paths
protected shared resources
risk level
agent profile
verification commands
```

The controller stores the graph as versioned JSON in a run-specific, git-ignored scratch directory
under `.superpowers/schoolorbit-workflow/`. The file is an execution artifact, not documentation;
remove it with the lane reports after final integration and review. The validator normalizes
repository-relative POSIX paths and rejects absolute paths, parent traversal, empty/root ownership,
unresolved globs, and duplicate task identifiers before comparing ownership.

`validate-work-graph.mjs` rejects a parallel wave when:

- two tasks own the same path or overlapping directory scopes;
- a task depends on another task in the same wave;
- more than one task touches a protected shared resource;
- a task uses a normal implementer profile for a high-risk domain;
- a task omits focused verification commands;
- ownership is broad, unresolved, or expressed through an unsafe repository-wide glob.

Rejected tasks are serialized or returned to planning; the validator never broadens ownership to
make a wave pass.

Protected shared resources include applied and new migration timelines, permission contracts and
generated registries, Rust/OpenAPI ownership and generated TypeScript DTOs, dependency lockfiles,
route or module registries that multiple lanes must edit, production topology, proxy templates,
deployment workflows, and any additional shared owner identified during planning.

## Worktree and Integration Strategy

Implementation never starts on `main` without explicit user consent. Detect existing isolation
first and prefer native worktree support. A single writable task uses one isolated feature
worktree. Two or more independent writable tasks use one branch and worktree per lane, all created
from the same current integration-branch head for that wave. The integration branch descends from
the approved base commit; later waves therefore include every earlier integrated dependency.

Each implementer may edit, test, and commit only in its lane worktree. Lane branches do not share
a Git index. At the wave barrier, the controller checks the changed-file set against declared
ownership, reviews each lane report, and integrates commits onto the integration branch in
dependency order. Review and verification run against the integrated state, not against isolated
lane claims.

An integration conflict stops that lane. The controller routes the conflict to the lane owner or
returns to planning when it exposes an invalid dependency or architecture assumption. It does not
guess through a conflict. Independent completed lanes remain recoverable but are not presented as
a completed feature while a required lane is blocked.

## Agent Result and Review Contracts

Every implementer returns one status: `DONE`, `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT`, or `BLOCKED`.
Its durable report records the assigned brief, base and head commits, changed files, focused tests,
command exit status, relevant output summary, self-review, and concerns. Summaries alone never
count as evidence.

The reviewer receives the approved requirements, lane or integrated diff, implementer report, and
binding global constraints. It reports both requirement compliance and code quality, with findings
classified as Critical, Important, or Minor. Critical and Important findings block progression.

Fix rounds one through three return to the original lane owner. Rounds four and five use a fresh
`gpt-5.6-sol` `max` implementer. Every round includes focused tests and a scoped re-review. After
five rounds, the controller adjudicates non-load-bearing findings with an explicit written ruling
and stops on any unresolved load-bearing defect. A finding that conflicts with the approved plan
is returned to the user immediately; neither the implementer nor reviewer chooses which governs.

## SchoolOrbit Safety Gates

- Read `.rules` before analysis or action and use `docs/README.md` to locate only relevant
  canonical documentation.
- Never edit an applied migration, including comments. Read the complete active timeline before
  proposing a database change and add a new sequential migration when approved.
- Change permission and API source contracts first, run their generators, and never edit generated
  artifacts directly.
- Treat frontend visibility as UX only; authorization changes require backend policy enforcement
  and allowed/denied tests.
- Never store, print, log, or include plaintext national IDs, credentials, tokens, database URLs,
  encryption keys, cookies, or raw sensitive request bodies in prompts, reports, test fixtures, or
  agent artifacts.
- Keep live permissions, sandbox choices, destructive operations, external writes, deployment,
  push, and pull-request creation behind their existing explicit authorization boundaries.
- Require the applicable `.rules` verification matrix plus focused tests. No agent report can
  waive a required command.

## Failure Handling

- `NEEDS_CONTEXT` receives the missing bounded context and resumes; the controller does not ask the
  agent to guess.
- `BLOCKED` triggers a check for missing context, inadequate model capability, excessive task size,
  or a defective plan. A plan defect or material ambiguity returns to the user.
- A failing test or unexpected result invokes `superpowers:systematic-debugging` before a fix is
  proposed.
- A failed lane prevents dependent lanes from starting or integrating. Independent lanes may
  finish, but the controller makes no whole-change completion claim.
- A changed user requirement pauses affected lanes and invalidates any approval whose scope or
  interfaces changed.
- Worktrees and lane branches remain intact until integration and final review are resolved so the
  work is recoverable.

## Skill Test Strategy

Develop the skill with RED-GREEN-REFACTOR as required by
`superpowers:writing-skills`.

### RED

Run fresh subagents without the new skill against disposable fixtures or worktrees. Preserve raw
JSON or text results in git-ignored scratch space and confirm baseline failures for combined
pressure scenarios, including:

- implementing before discussing and obtaining approval;
- treating a tiny request as permission to skip planning;
- dispatching overlapping writers to save time;
- editing an applied migration or generated contract under deadline pressure;
- continuing after implementation evidence changes the approved scope;
- trusting a subagent success claim without inspecting its diff or test output.

### GREEN

Add the smallest skill, agent configuration, ownership validator, and repository guards that
address observed failures. Re-run the same scenarios with the skill enabled. The expected behavior
is an approval stop, valid routing, serialized protected resources, isolated lanes, and
evidence-based verification.

### REFACTOR

Capture any new rationalizations, tighten the applicable condition or output contract, and repeat
the scenarios. Behavior-shaping wording is compared against a no-guidance control with multiple
fresh-context samples before the skill is treated as stable.

## Repository Tests

`schoolorbit-agent-workflow.test.mjs` statically verifies:

- exact skill frontmatter and implicit invocation metadata;
- multi-agent enablement and the three-thread cap;
- registration of every agent role;
- Planner and Reviewer use `gpt-5.6-sol` with `max` effort and read-only sandboxes;
- Explorer, Implementer, high-risk Implementer, and Verifier match the approved model matrix;
- the approval state precedes every implementation state;
- the work-graph validator accepts independent paths and rejects overlaps, dependencies in the
  same wave, protected-resource sharing, missing verification, and wrong risk routing;
- `.rules` and the documentation allowlist recognize the repository skill without permitting
  unrelated Markdown.

The implementation also forward-tests the completed skill with fresh subagents and manually
reviews every flagged result. Test agents operate only in disposable fixtures or worktrees and
never receive real secrets or national IDs.

## Verification

Run focused checks first, then the repository-owned gates:

```bash
node --test frontend-school/tests/static/documentation-policy.test.mjs
node --test frontend-school/tests/static/schoolorbit-agent-workflow.test.mjs
cd frontend-school && npm run test:static
git diff --check
git status --short
```

Also run the skill creator's structural validator when available and complete the documented
without-skill/with-skill forward scenarios. Read the final diff and verification output directly;
do not infer success from an agent summary.

## Acceptance Criteria

- Every SchoolOrbit mutation request reaches an approval-ready plan before any implementation
  write.
- No implementer starts until the user explicitly approves the current plan and, when required,
  reviews its written artifact.
- Planner and Reviewer always run on `gpt-5.6-sol` with `max` reasoning.
- Independent lanes may execute concurrently only after deterministic work-graph validation and
  only in separate worktrees.
- Shared contracts, migrations, generated artifacts, lockfiles, and deployment owners are
  serialized.
- Every task and integrated change receives review plus fresh verification evidence.
- Any material plan change returns to user approval.
- The repository policy, static tests, and skill forward tests all pass without weakening an
  existing SchoolOrbit invariant.
