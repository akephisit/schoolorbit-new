# Academic receipt recovery implementation plan

Approved input: [migration-149 recovery design](../specs/2026-10-31-academic-receipt-recovery-design.md),
recording the Senior/Data-approved contract. Follow [the development rules](../../../.rules).
The actual cause of the three production mismatches remains unknown; implementation corrects a
source-confirmed invalid transition premise while preserving accepted receipt facts.

## Ownership and execution order

The Senior owns technical decisions, shared Git/checkpoints, integration and production operations.
Backend owns the runner, immutable new migration 149, Academic Core reconciliation and disposable
database fixtures; it is the sole Cargo owner. Pipeline owns the six-check consumer gate and
canonical operational/testing documentation when assigned. Writers use disjoint assigned paths;
neither edits applied migrations 147/148, SQLx history, credentials or real tenant data.

1. **Backend: freeze the finite runner interface.** Trace current migration/connection lifetimes,
   SQLx's native migration/savepoint execution and permission/grant consumers. Define the exact
   147, existing-148 and replayed-149 branches, preflight shape/dependency/trigger requirements,
   lock cohort, staging namespace, failure/cancellation behavior and evidence schema against the
   approved design. Preserve the old migration bytes and existing public provider direction.
2. **Backend: implement preparation and atomic transition.** On the verified locked 147 branch,
   admit only mask-1 lower-count cases within distinct output cardinality. Preserve the original
   physical count column; capture all original receipts and prove reconstruction by bidirectional
   `EXCEPT ALL`, counts and SHA before the unchanged native 148 runs. Keep preparation, native
   148 and new 149 in one outer transaction/connection, with no preparation DML on other domains.
3. **Backend: finalize migration 149 and readiness.** Restore the original column, prove all
   original receipt JSON minus new `starts_on` and null legacy date intent, and reuse
   `pg_temp.academic148_snapshot` to exclude nonreceipt restoration changes. Store immutable
   bounded phase-linked evidence, remove every staging/helper object, and preserve existing-148
   receipts exactly. Release the committed connection before permission/grant work so pool size
   one remains supported. Extend the Academic Core provider to six version-149 checks without
   an ongoing timestamp/count scan.
4. **Pipeline: consume the provider and document recovery.** After the Backend contract is
   stable, require `ACADEMIC_CONTEXT_RECEIPT_RECOVERY_EVIDENCE` alongside the existing five
   checks for every tenant; change the context version from 148 to 149 and exact check count
   from five to six, with status pending below 149. Preserve maintenance, migration completion, immutable artifact and
   compiled-schema fences. Update canonical Operations/Testing with the finite protocol,
   ambiguous-commit reconciliation and selected verification; do not add a second executor or
   diagnostic-only deployment prerequisite. Make existing observer preparation recognize the
   trusted 001–149 source inventory and derive its unchanged 001–148 manifest. Keep that
   optional read-only query bounded to 147/148 and document version-149 refusal, rather than
   silently breaking the registered flag or introducing a business fallback.
5. **Senior: integrate and freeze.** Inspect the complete candidate and preservation evidence;
   checkpoint only agreed paths. Then pause all writers and obtain independent Rules/Security
   Review and QA on the same candidate. Assign any repair back to its owner and refresh affected
   evidence before readiness, merge and production acceptance.

## Verification to select before implementation

Backend first adds the source-confirmed historical delete/reapply same-UUID reproducer. Its tests
must establish why unchanged 148 refuses the history and why preserving `N` through the approved
projection/finalization yields the intended date-free legacy request without altering facts.

Use the owning disposable database runner and focused migration/runner/Academic Core integration
filters from [Testing](../../TESTING.md); record the exact existing/new test commands before
execution. Cover:

- Exact receipt JSON multiset/count/SHA preservation and nonreceipt transitive-graph equality;
  original column identity/default/checks; no surviving staging/helper objects.
- Rejection of every other mask, `N` above distinct nonnull output cardinality, malformed or
  unsupported history, unexpected shape/triggers/dependencies, drift and stale/incomplete proof.
- Failures and cancellation after projection, after native 148 and inside 149, with fresh-history
  reconciliation; a second connection never sees committed 148/staging on the recovery path.
- Existing-148 nonnull `starts_on`, repeated 149, fresh provisioning and one-connection privileged
  pools; post-commit permission/grant failures preserve truthful committed history.
- All six readiness checks, especially pending status below 149, missing/invalid recovery evidence
  and residual staging; historical-observer preparation after migration 149 is added, with its
  schema boundary retained.

Pipeline runs the affected executable migration-completion and deployment guards, then the owning
`./scripts/pipeline verify --scope deployment` checks when its consumer changes. Reuse unaffected
observer evidence; schema-v2 observation is not proof of migration execution. Documentation changes
require the existing documentation-policy check. Always inspect final diff/status and whitespace.
No fixture may target production, patch SQLx records or weaken 148 to obtain a passing result.

## Independent acceptance and operational completion

The Reviewer independently checks the full transaction boundary, native-148 immutability, exact
preservation proofs, finite staging lifetime, truthful evidence/readiness, pool/connection lifetime
and refusal/recovery paths. QA selects and runs the affected tests independently, including failure,
cancellation and separate-connection visibility; writer results do not replace either acceptance.

The Senior follows normal latest-main/PR gates and merge-commit delivery, then the selected normal
Pipeline under universal maintenance. Require every tenant at exact version 149 with all six
checks, the selected frontend/backend acceptance and authenticated smoke before opening maintenance.
If a client disconnects ambiguously, reconcile fresh history/evidence first. Roll forward through
the canonical runner; never restore an older writer or apply a manual tenant/history repair.
Record actual outcomes and remaining blockers in the PR/chat. Once the implementation PR records
the completed outcome, retire these workflow artifacts under the repository documentation policy.
