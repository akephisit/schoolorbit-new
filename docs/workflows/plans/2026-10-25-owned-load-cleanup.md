# Owned load cleanup implementation plan

Status: Senior approved implementation under the linked execution contract.

Design: [Owned load fixture cleanup](../specs/2026-10-25-owned-load-cleanup-design.md).

## Ownership and baseline

- Senior owns Git, source checkpoints, contract decisions, acceptance, and authorized delivery.
- Delegated Pipeline writer uses the assigned shared checkout and branch
  `codex/operations-owned-load-cleanup`, based on `7cb64572ee9ef2f95d086db08fa6f2d57bae43ff`.
- Independent Reviewer and QA inspect the complete frozen implementation and actual evidence.
- Preserve the original 64 input hashes and private prepared cleanup candidate. No new load,
  production SQL, credentials, runtime grants, application behavior, or schema changes are part
  of implementation preparation.

## Sequence after contract approval

1. Confirm the fixed operation catalogue and inert staging boundary. Use the approved active-tenant
   Admin owner-connection transport. Preserve the reviewed SQL bytes and conservative auth cutoff;
   capture a separate fresh ActualExecutionBinding under the production lock after the workflow
   PR deploys. Require the exact reviewed image and the
   full schema-138 timeline. Record the reconciliation-only recovery semantics.
2. Add the explicit Operations choice and callable workflow with main-only trust, shared
   `schoolorbit-production` concurrency, cancellation disabled, and bounded typed inputs.
3. Add the narrow remote driver and shared cleanup interlock. Enumerate every actual mutation
   owner and insert refusal before its first production mutation, including retention. Persist
   journal/intent before dispatch; never infer rollback from controller failure.
4. Validate the private bundle against trusted catalogue hashes. Resolve the fixed tenant's owner
   connection through Admin, verify role/database/schema identity, and keep credentials remote
   and private. Use no supplied executable or arbitrary query transport.
5. Implement fresh preflight approval, atomic cleanup dispatch once, private durable commit and
   reconciliation receipts, and the at-least-300-second stable-process watch. Preserve the
   original watch contract separately from the new schema-138 snapshot. Reconcile-only recovery
   cannot execute DML or clear an unresolved interlock.
6. Add the narrow `.rules` owner-job boundary, focused tests, canonical Operations/Testing recipes,
   and any necessary planner impact
   classification. Select verification by actual changed owners. Do not add CI test execution
   or local-test receipt upload/attestation machinery.
7. Pause for a Senior checkpoint, then freeze all inputs for independent Review and QA. Repair
   findings through the writer and rerun affected checks. Remove these two workflow artifacts
   after the implementation PR records the completed outcome.

## Required failure cases

Reject untrusted refs, wrong/unknown operation or action, arbitrary input keys, traversal,
symlinks, file/hash/mode/owner drift, stale accepted state, wrong tenant/role, wrong migration
timeline, changed source/process, unresolved foreign journals, or consumed cleanup intent.

Exercise cancellation before dispatch, during SQL, after possible commit and during observation;
controller disappearance must retain a fail-closed interlock. Prove every production mutator
refuses an unresolved cleanup, including when GitHub has released its concurrency group.
Reconciliation must never replay DML or treat an unknown result as a successful cleanup.

Reuse the reviewed SQL cases with sanitized disposable data: exact cleanup and actor/audit/catalog
preservation; control or manifest drift; unrevoked/post-start auth; outside CASCADE and composite
SET NULL references; busy job NOWAIT; outside/audit/password mutation rollback; tampered original
watch proof; postcheck and replay refusal. Add transport credential confidentiality, exact
original watch ID/digest/counter-floor validation, monotonic bookkeeping acceptance, watch
resurrection/business drift rejection, and the 300-second duration/source continuity boundary.

## Runnable verification gates

Use the pinned local Node toolchain and the repository's owned Docker fixtures. Run focused new
Node/Bats tests while repairing their owner, then the required deployment suite once the final
candidate is stable:

```bash
node --test scripts/tests/pipeline*.test.mjs
node --test frontend-school/tests/static/documentation-policy.test.mjs
./scripts/pipeline verify --scope deployment
```

The deployment suite owns ShellCheck, shfmt, installer/remote Bats, planner/database fixtures,
workflow/source guards, Compose validation, and actionlint. Avoid repeating its constituent
checks without an additional coverage need. Add the new test recipe to `docs/TESTING.md` and
run the bounded PostgreSQL 18.6/schema-138 rehearsal locally; no Cargo build is planned for
workflow/script-only changes. Record actual failures and their repairs, tested hashes, commands,
and unavailable dependencies. Existing private thirteen-case SQL evidence is preliminary reuse,
not proof that the new workflow, transport, interlock, or watcher works.

## Completion boundary

Implementation is complete only after independent Review/QA accept the full frozen candidate.
Production cleanup remains a separate Senior operation through the supported shared-lock path,
using freshly approved inputs and actual private receipts plus the completed observation.
