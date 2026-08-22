---
name: schoolorbit-workflow
description: Use when building, changing, fixing, refactoring, or otherwise modifying files or behavior in the SchoolOrbit repository.
---

# SchoolOrbit Workflow

## Core Contract

Treat `.rules` as the single authoritative development standard. Use this skill to control the
workflow; never use it to weaken a repository rule.

Never mutate repository state without a current plan that the user explicitly approved. This
includes source, tests, configuration, documentation, generated artifacts, commits, and delegated
writes. A read-only request authorizes inspection and reporting only; it never authorizes writes.
For multi-file or high-risk work, the first approval authorizes only the agreed plan artifacts and
the second approval authorizes implementation.

## Readiness Gate

On every explicit or implicit invocation, inspect the runtime dependencies before doing anything
else:

- `.agents/skills/schoolorbit-workflow/agents/openai.yaml` contains final interface metadata and
  `allow_implicit_invocation: true`.
- `.codex/config.toml` registers all six profiles, and these files exist:
  `schoolorbit-planner.toml`, `schoolorbit-explorer.toml`, `schoolorbit-implementer.toml`,
  `schoolorbit-high-risk-implementer.toml`, `schoolorbit-reviewer.toml`, and
  `schoolorbit-verifier.toml`.
- `.agents/skills/schoolorbit-workflow/scripts/validate-work-graph.mjs` exists and is runnable.

If metadata is bootstrap-disabled, a registration is absent, a role file is absent, or the
validator is unavailable, return `BLOCKED` with the missing dependency before planning,
delegation, or mutation. Do not silently downgrade to an incomplete workflow.

## Workflow State Machine

After classification, follow only the applicable branch.

### Standard and Full workflows

`DISCOVER` → `DRAFT_PLAN` → `AWAIT_APPROVAL` → conditional `RECORD_PLAN` →
`EXECUTE_WAVES` → `INTEGRATE` → `REVIEW_FIX` → `VERIFY` → `COMPLETE` or `BLOCKED`.

`NORMAL` work retains this Standard Workflow exactly. `HIGH_RISK` work retains this Full Workflow
and every current safety requirement. For multi-file or high-risk work, transition from
`RECORD_PLAN` back to `AWAIT_APPROVAL` for the second review before `EXECUTE_WAVES`.

### Fast Path

Only an approved `TRIVIAL` mutation may use this exact sequence:

`DISCOVER` → `DRAFT_PLAN` → `AWAIT_APPROVAL` → `FAST_EXECUTE` → `FAST_VERIFY` →
`COMPLETE` or `BLOCKED`.

If scope expands, a protected resource appears, behavior exceeds the approved expectation,
verification fails and requires logic changes, a security, API, database, authentication,
authorization, or permission concern appears, multiple domains become necessary, or more than one
writer is needed, stop immediately. Invalidate the Fast Path, return to `DRAFT_PLAN`, reclassify
the work as `NORMAL` or `HIGH_RISK`, and require fresh approval before mutation resumes. A material
scope change from any later state likewise invalidates approval. Never infer, reorder, or skip a
state.

## Classify the Request

Classify the requested outcome, not merely its verbs.

- Read-only: explain, inspect, diagnose, review, compare, or report without changing repository or
  external state. Stop after evidence-backed reporting.
- Mutation: build, add, fix, refactor, rename, generate, update, commit, or otherwise cause a file,
  behavior, branch, external system, or durable artifact to change.

When ambiguous, continue read-only discovery and ask for the missing intent before proposing any
write. Do not convert a diagnosis request into an implementation task.

### Mutation risk classification

Classify every mutation as `TRIVIAL`, `NORMAL`, or `HIGH_RISK`, fail closed, and apply
`HIGH_RISK` precedence before considering any lower class.

- `HIGH_RISK`: any authentication, authorization, permissions, migration, API contract, PDPA or
  sensitive-data, realtime identity or session-security, deployment, security, or other protected
  or load-bearing resource. These concerns can never enter the Fast Path.
- `TRIVIAL`: allowed only when every eligibility condition is affirmatively satisfied. The scope
  is small and single-area; there is no behavioral, API, database, or security impact; and there
  is no protected resource, generated contract, migration, auth, authorization, permission, PDPA
  or sensitive data, realtime identity or session security, deployment or infrastructure,
  dependency or lockfile, multiple writer lanes, or material architectural decision. Eligible
  examples are a typo, copy or text, a small comment or documentation correction,
  CSS/presentation-only work, or a rename or local cleanup only when behavior preservation is
  proven.
- `NORMAL`: every mutation not classified `HIGH_RISK` or proven `TRIVIAL`. Any uncertainty about
  scope, impact, eligibility, or behavior immediately becomes `NORMAL`.

**REQUIRED SUB-SKILL:** Use superpowers:brainstorming for creative or behavior-changing design.

## Discover

Read `.rules`, then use `docs/README.md` to locate only relevant canonical documentation. Trace
the current implementation directly: follow re-exports, call sites, route registration, migration
history, generated-contract ownership, tests, and deployment paths before drawing conclusions.
Map impact across backend, frontend, database, permissions, API contracts, realtime, security and
PDPA, deployment, documentation, and tests.

Before implementation approval, discovery is filesystem-read-only. Do not run builds, tests,
linters, formatters, generators, package managers, installers, auto-fixers, or any tool that can
create caches or artifacts. Derive future commands from source, configuration, and documentation,
then defer their execution until implementation approval. If a command's write behavior is
uncertain, do not run it.

Bound each investigation by domain and requested evidence. Parallelize only independent read-only
investigations; reconcile their evidence in the controller before drafting the plan.

### Bounded context contract

An Explorer must not send the entire conversation or repository context. A Planner receives only
the reconciled evidence packet, and an Implementer receives only its bounded task brief. A
Reviewer receives the approved requirements, diff, work graph, and verification evidence. Agents
must not repeat repository exploration or discovery without a concrete reason, such as a named
missing fact or changed evidence. Prefer file paths, symbols, and evidence over copied file
contents.

For `TRIVIAL` work, the controller performs bounded read-only discovery and may draft the bounded
in-chat plan directly. For `NORMAL` and `HIGH_RISK` work, when Explorer work is used, the
controller builds a reconciled evidence packet containing the requested outcome, direct facts with
paths or symbols, impact and risk, ownership candidates, verification sources, and unresolved
decisions. Spawn one bounded Planner using `fork_turns: none` and give it only that packet plus the
required plan fields. The Planner must not repeat Explorer discovery. If the packet is incomplete,
it returns `NEEDS_CONTEXT` naming the exact missing fact instead of reopening repository-wide
investigation.

**REQUIRED SUB-SKILL:** Use superpowers:dispatching-parallel-agents for independent read-only investigations.

## Plan Contract

For `NORMAL` and `HIGH_RISK`, use one bounded Planner to turn the reconciled evidence packet into
the plan. Defer implementation-only skills, including worktree setup, TDD, code review, and
verification, until their corresponding state after implementation approval. During `DRAFT_PLAN`,
name those future checkpoints without loading or executing their skills.

Present a concrete plan in chat before any mutation. Include:

- requested outcome, in-scope and out-of-scope work, assumptions, and unresolved decisions;
- the impact matrix and risk classification, including why each high-risk boundary is high-risk;
- numbered tasks with dependencies or waves, exact owned paths, protected resources, assigned
  profile, and normal or high risk;
- for multiple writer lanes, the order in which the controller audits and integrates them serially
  before starting the independent Reviewer;
- source-first ownership for generated permission and API contracts;
- exact focused and repository verification commands; and
- external or destructive actions that require separate authorization.

Every writer must have disjoint ownership. Treat migrations, permission contracts and generators,
API contracts and generators, lockfiles, route registries, deployment owners, and security or
identity surfaces as protected resources that require a single serialized owner. End by asking the
user to approve the exact current plan.

### TRIVIAL plan contract

The controller may draft a `TRIVIAL` plan without a Planner. Keep it concrete and in chat: state
the bounded outcome, affirmative Fast Path eligibility, exact owned paths, focused verification,
and escalation conditions. Do not create design or implementation plan artifacts, a multi-agent
work graph, or an implementation task graph. Explicit approval of this exact plan remains required
before any mutation.

**REQUIRED SUB-SKILL:** Use superpowers:writing-plans for multi-file or high-risk implementation plans.

## Approval Gate

Require an unambiguous affirmative response tied to the exact current plan, such as “I approve
this exact plan” or “อนุมัติแผนนี้ ให้เริ่มทำได้”. A question, silence, partial preference, general
acknowledgement, approval of an older plan, or permission to keep investigating is not approval.

Approval is required for every mutation, including a one-line or urgent change. Before approval,
allow only read-only discovery and an in-chat plan: do not start a writer, create scratch files,
record plan artifacts, edit, generate, commit, or integrate.

Invalidate approval when scope, owned paths, affected system domains, protected resources, API or
database behavior, permission or auth impact, risk, dependencies, or verification materially
changes. Explain the delta, revise the plan, and return to `AWAIT_APPROVAL`. Never treat approval
as transferable between plans.

## Plan Artifacts

Keep a `TRIVIAL` plan in chat and create no plan artifacts. For `NORMAL`, keep a small, low-risk,
single-area plan in chat; one explicit approval may authorize its implementation. For multi-file or
`HIGH_RISK` work, discuss and name exactly two dated Superpowers artifacts in chat: one design
under `docs/superpowers/specs/` and one implementation plan under `docs/superpowers/plans/`. The
first approval must name and authorize that exact pair. Record both or neither, then show both
recorded artifacts for a second user review and wait for explicit implementation approval. If
either exact path is missing from the approved plan or recorded pair, return to `DRAFT_PLAN`,
repair the pair, and obtain fresh first approval; never advance to the second approval gate with
only one artifact.

The first approval never authorizes staging or committing; leave both artifacts untracked until
explicit implementation approval. Do not include `git add`, a local commit, or any other Git-index
mutation in the first-gate plan or approval request.

Do not create status Markdown, skill references, per-feature README files, or other plan copies.

## Model Routing

Use this exact role matrix and pass the role, model, and reasoning effort explicitly whenever an
agent is started:

| Role | Model | Effort | Sandbox default |
|---|---|---|---|
| Planner | `gpt-5.6-sol` | `max` | read-only |
| Explorer | `gpt-5.6-terra` | `xhigh` | read-only |
| Implementer | `gpt-5.6-sol` | `xhigh` | workspace-write |
| High-risk Implementer | `gpt-5.6-sol` | `max` | workspace-write |
| Reviewer | `gpt-5.6-sol` | `max` | read-only |
| Verifier | `gpt-5.6-terra` | `high` | workspace-write |

Route authentication, authorization, permissions, migrations, API contracts, PDPA and sensitive
data, realtime identity, deployment, security, and other load-bearing work to the High-risk
Implementer. The Planner or Reviewer may conservatively elevate any task.

If a named custom role is unavailable, inline that role's complete `developer_instructions` in the
brief and still set its exact model and effort. Never use silent inheritance. If the required model
or effort cannot be selected, return `BLOCKED` rather than substituting a weaker profile.

## Validate the Work Graph

For `NORMAL` and `HIGH_RISK`, after implementation approval, record the bounded graph at
`.superpowers/schoolorbit-workflow/work-graph.json` with `{ "version": 1, "tasks": [...] }`.
Each task must declare `id`, `wave`, `dependencies`, `ownedPaths`, `protectedResources`, `risk`,
`agentProfile`, and `verification`.

Run:

```bash
node .agents/skills/schoolorbit-workflow/scripts/validate-work-graph.mjs \
  .superpowers/schoolorbit-workflow/work-graph.json
```

Do not start writers unless the validator exits zero. Fix the plan or ownership graph, not the
validator, when it rejects overlap, unsafe paths, excessive concurrency, dependencies, profile
routing, or protected-resource ownership.

The Fast Path creates no work graph.

## Execute Approved Work

### NORMAL and HIGH_RISK execution

For `NORMAL` and `HIGH_RISK`, create an isolated worktree and therefore a separate Git index for
every writer lane. Start only dependency-ready tasks in the same validated wave, with at most three
active agents total. Never put independent writers in a shared worktree, and serialize every
protected resource. Give each worker a bounded brief containing the approved task, base commit,
dependencies, owned paths, protected resources, exact profile, verification commands, stop
conditions, and required status report.

Implement with tests first, make the smallest coherent change, commit the lane, and prohibit edits
outside ownership. When a bug or unexpected failure appears, establish its root cause before
changing production behavior or tests.

**REQUIRED SUB-SKILL:** Use superpowers:using-git-worktrees before implementation and for parallel lanes.

**REQUIRED SUB-SKILL:** Use superpowers:test-driven-development for features and bug fixes.

**REQUIRED SUB-SKILL:** Use superpowers:systematic-debugging for bugs or unexpected failures.

Run read-only roles in isolated contexts too. A live parent sandbox can override a profile's
declared default, so capture a pre- and post-task `git status --short` and changed-file audit.
Reject the task and do not integrate it if a read-only role makes any write. Never rely on a role's
success claim without examining its returned commit and evidence.

Do not use `superpowers:subagent-driven-development` as the parallel execution engine because it
forbids concurrent implementers. This SchoolOrbit worktree-wave controller owns parallel writer
execution; use the other required Superpowers at their named checkpoints.

### TRIVIAL Fast Path execution

At `FAST_EXECUTE`, the controller starts exactly one `schoolorbit_implementer` for the exact owned
paths in the approved bounded brief. A separate worktree is not required when this sole writer has
no other isolation requirement. Do not add a Planner, design or implementation plan artifacts,
work graph, independent Reviewer, separate Verifier, or multi-round review by default. Never infer
authority to stage or commit.

The Implementer makes only the approved behavior-preserving edit and stops at the first escalation
condition. If implementation needs another path, domain, writer, protected resource, material
decision, or behavior beyond the approved expectation, it reports the condition without widening
scope or continuing the Fast Path.

## Integrate and Review

### NORMAL and HIGH_RISK review

For every completed `NORMAL` or `HIGH_RISK` lane, compare its base-to-head diff with `ownedPaths`,
inspect generated and sensitive-data boundaries, verify its commit and report, and reject unowned
changes. Integrate eligible lanes serially into the controller worktree in dependency order.
Resolve no semantic conflict by guesswork; return it to the responsible owner or revise the plan.

After all approved tasks are integrated, start an independent Reviewer using `gpt-5.6-sol` at
`max`. Give it the approved requirements, base and head commits, full diff, work graph, and
verification evidence. Require findings categorized as Critical, Important, or Minor with file
references. Re-review after every fix round.

Rounds one through three return findings to the original owner. Rounds four and five use a fresh
`gpt-5.6-sol` `max` implementer with explicit ownership. Unresolved load-bearing defects block
after round five; never relabel them as concerns merely to complete.

For `NORMAL` and `HIGH_RISK`, **REQUIRED SUB-SKILL:** Use superpowers:requesting-code-review for
task and integrated review.

### TRIVIAL Fast Path review boundary

Fast Path work has no integration lane, independent Reviewer, or review-fix rounds. The controller
still audits the sole Implementer's ownership, evidence, final diff, and status directly. Any
finding that requires logic changes or expands scope triggers Fast Path escalation and fresh
approval.

## Verify and Finish

### NORMAL and HIGH_RISK verification

For `NORMAL` and `HIGH_RISK`, re-read the current `.rules` and run every applicable focused command
and verification-matrix command fresh against the integrated worktree. Capture exit codes and
relevant output. Inspect the base-to-head diff directly, confirm only approved files changed,
verify generated artifacts came from their source, audit for secrets or plaintext national IDs,
run `git diff --check`, and inspect `git status --short`.

Do not accept stale CI, a worker's summary, or a Reviewer opinion as verification. A Verifier may
use workspace-write only for test-created build or cache artifacts; audit and reject source,
configuration, snapshot, or expectation changes. Require separate authorization before push,
pull-request publication, merge, deploy, destructive cleanup, or other external state changes not
already named in the approved plan.

**REQUIRED SUB-SKILL:** Use superpowers:verification-before-completion before success claims.

**REQUIRED SUB-SKILL:** Use superpowers:finishing-a-development-branch after verified implementation when the user chooses integration.

### TRIVIAL Fast Path verification

At `FAST_VERIFY`, run the approved focused checks and any applicable deterministic repository
validation fresh. Prefer deterministic validation over another LLM when repository rules fully
establish the result; do not start a separate Verifier. Capture commands, exit statuses, and
relevant output, then inspect `git diff --check`, the complete final diff against the base, and
`git status --short`. Confirm that only exact owned paths changed and no exclusion or escalation
condition appeared. A failed check that requires logic changes invalidates the Fast Path and
requires reclassification, a revised plan, and fresh approval.

## Status Contract

Every delegated task returns exactly one status:

- `DONE`: approved scope is complete and all named checks passed.
- `DONE_WITH_CONCERNS`: approved scope is complete and checks passed, with explicit non-blocking
  concerns.
- `NEEDS_CONTEXT`: the bounded brief lacks information required to proceed safely.
- `BLOCKED`: a readiness, ownership, dependency, safety, test, or load-bearing review condition
  prevents completion.

Every task report includes the base commit, head commit, changed files, focused commands and exit
statuses, output summary, self-review, and concerns. For `NEEDS_CONTEXT` or `BLOCKED`, also state
the exact missing input or failed condition and whether any write occurred. The controller verifies
these fields and evidence independently; a delegated `DONE` is never the controller's completion
claim.

## Quick Reference

| State | Lead role | Required action |
|---|---|---|
| `DISCOVER` | Explorer / controller | Read rules and collect bounded direct evidence. |
| `DRAFT_PLAN` | Controller or Planner `sol/max` | Controller drafts `TRIVIAL`; Planner handles `NORMAL` / `HIGH_RISK`. |
| `AWAIT_APPROVAL` | Controller | Wait for explicit approval; permit no write. |
| `FAST_EXECUTE` | One `schoolorbit_implementer` | Edit only exact owned paths; stop on escalation. |
| `FAST_VERIFY` | Controller / Implementer | Run deterministic focused checks; inspect diff and status. |
| `RECORD_PLAN` | Controller | For `NORMAL` / `HIGH_RISK`, record approved artifacts when required, then re-ask. |
| `EXECUTE_WAVES` | Implementer | For `NORMAL` / `HIGH_RISK`, validate graph; use isolated worktrees, TDD, and max three. |
| `INTEGRATE` | Controller | For `NORMAL` / `HIGH_RISK`, audit ownership and integrate lanes serially. |
| `REVIEW_FIX` | Reviewer `sol/max` | For `NORMAL` / `HIGH_RISK`, independently review, fix, and re-review. |
| `VERIFY` | Verifier / controller | For `NORMAL` / `HIGH_RISK`, run fresh checks and inspect the final diff. |
| `COMPLETE` / `BLOCKED` | Controller | Report evidence or the exact stopping condition. |

## Example

Suppose an approved attendance change has two independent normal-risk lanes and one protected API
contract task:

- Wave 1 task `backend-domain` owns only the attendance service and focused Rust tests in its own
  worktree. Task `frontend-view-model` owns only the attendance view model and its TypeScript tests
  in a second worktree. They have no shared protected resource and may run concurrently.
- Integrate both Wave 1 commits serially after ownership audits.
- Wave 2 task `api-contract` depends on both, is high risk, uses the High-risk Implementer, and
  exclusively owns `backend-school/src/api_contract.rs`, `contracts/openapi/`, and
  `frontend-school/src/lib/api/generated/` with protected resource `api-contract`. It updates the
  source first and runs the generator; no other writer runs beside it.
- Integrate Wave 2, run the independent Reviewer at `gpt-5.6-sol` `max`, complete any reviewed fix
  rounds, then run fresh integrated verification before reporting completion.

## Common Rationalizations

- `"done"`, “one line”, or “urgent” never permits skipping the in-chat plan or explicit approval.
- “independent writers”, “shared worktree”, “inert fixtures”, or “disk or time constraints” never
  permits pre-approval writes, overlapping ownership, or multiple writers in one worktree.
- A “generic frontier specialist” or generic “security reviewer” does not satisfy explicit
  Planner, Reviewer, or High-risk Implementer model-and-effort routing.
- “The Planner should independently verify everything” ignores the controller's reconciled
  evidence packet; repeating Explorer discovery breaks bounded delegation and adds no approval
  safety.
- “Recording the plan includes a local commit” expands the first gate; plan artifacts remain
  untracked until implementation approval.
- “Small”, “single file”, or a familiar example does not prove `TRIVIAL`; every eligibility
  condition must be affirmative, and uncertainty becomes `NORMAL`.
- “List only”, “dry run”, or “just checking” does not make a build, test, linter, formatter,
  generator, package manager, installer, or auto-fixer read-only when it can create a cache or
  artifact.

## Red Flags

Stop and return to the appropriate earlier state when any of these appears:

- implementation, scratch creation, generation, or writer delegation before approval;
- a pre-approval build, test, lint, format, generation, install, auto-fix, cache, or artifact;
- reliance on approval after a material scope or risk change;
- overlapping owned paths or shared protected resources in one wave;
- parallel writers sharing a worktree or Git index;
- direct edits to generated permission or API artifacts;
- edits to an applied or legacy migration;
- Fast Path scope expansion, protected resources, unexpected behavior, verification requiring
  logic changes, multiple domains, or another writer;
- read-only roles that changed files;
- integration outside dependency order or without an ownership audit; or
- a completion claim without fresh verification and direct diff evidence.
