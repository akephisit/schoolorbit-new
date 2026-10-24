# Storage namespace rollout implementation

Follow the approved [design](../specs/2026-10-25-storage-namespace-rollout-design.md) and current
repository rules. Senior owns Git, production inspection and release dispatch. The Pipeline writer
owns workflow/scripts/documentation; the Backend writer owns Rust/Admin contracts. Writers share
one branch and pause at checkpoints. Independent Review and QA accept the integrated candidate.

1. **Save the contract checkpoint.** Record the headless command names, strict attestation JSON,
   safe output schemas, all-tenant/empty-inventory behavior and migration-attempt recovery boundary.
   Do not introduce another migration engine, tenant inventory or release binary.
2. **Implement headless providers.** Extract/reuse the namespace CLI owner for the main executable
   and standalone wrapper. Add canonical worker-free tenant migration, authenticated complete
   inventory/context/readiness and explicit attestation commands. Extend internal migration
   credential access with the authorized inactive-tenant opt-in. Gate ordinary startup before
   application worker registration. Preserve default internal authorization and schema history.
3. **Wire deployment.** Validate optional manual input through Pipeline and reusable release calls.
   Run the exact prepared image with private allowlisted configuration. Verify maintenance, capture
   and preserve old routing/image context, stop/verify/remove old writers, mark migration attempt,
   migrate, optionally attest only on explicit dispatch, check complete readiness and start matching
   writers. Retain existing post-start readiness, cutover audits, smoke and global acceptance.
4. **Test failure and recovery.** Exercise the actual shell adapter with synthetic dependencies.
   Reject malformed/unknown inputs, wrong routing/inventory/image proof, missing old evidence,
   migration/check failures and incomplete inventory. Prove normal automatic runs never bind and
   no server starts before successful namespace readiness. Preserve old artifacts and prohibit old
   restart after migration attempts. Test empty/inactive inventory and private output handling in
   the owning Rust fixtures. Update existing source guards only for justified stronger invariants.
5. **Document and checkpoint.** Update Operations and Testing around the real packaged main-command
   recipe, explicit assertion/evidence procedure and compatible roll-forward recovery. Record exact
   checks/results and remaining operational evidence. Freeze source for independent Review and QA.
6. **Complete the authorized release.** Senior verifies latest-main ancestry, integrates through the
   normal Pipeline and observes trusted preparation/maintenance gates. Obtain real old routing and
   complete inventory context, supply the independently authorized historical evidence only in the
   explicit attestation dispatch, and verify all migrations/namespace readiness before writers.
   Finish ordinary ready/smoke/acceptance and confirm traffic opens. Never infer historical routing,
   bypass a failed gate or describe an incomplete deployment as successful.
