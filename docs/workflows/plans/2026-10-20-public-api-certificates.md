# Public API certificate implementation

Design: [public API certificates](../specs/2026-10-20-public-api-certificates-design.md).
Base: `5576603a3d214d1107000e4da4b9dbd4765e3137`, tree
`00c2ad111f2212a0c590d47924d168b9fa18c3e7`.

1. Pipeline writer implements the shared rootless ACME owner and focused executable
   fixtures. Senior saves a meaningful review checkpoint and opens the draft PR.
2. The same writer replaces installer Origin CA issuance/checkpoints, direct trust
   consumers, DNS-only target/resolver checks and provider fixtures. Preserve
   installer confirmation, snapshot drift, resume and rollback guarantees.
3. Wire scheduled Maintenance renewal and explicit Operations migration through the
   existing production lock. Reuse maintenance/readiness/authenticated smoke owners;
   keep application release and baseline publication in Pipeline.
4. Update `.rules`, Operations, Podman setup and Testing at their canonical owners.
   Run `./scripts/pipeline verify --scope documentation,deployment`, focused new TLS
   cases and `git diff --check`; report actual results and tested inputs.
5. Freeze the candidate for separate Rules/Security review and QA. The writer repairs
   findings; Senior alone stages, commits, pushes, integrates and operates production.
6. Senior disables automatic rollout gates, integrates only after latest-main
   acceptance, verifies exact main preparation, performs explicit certificate
   migration under the production lock, runs the normal prepared Pipeline, verifies
   both public-trust API targets, cuts over only both API DNS records, verifies
   public/authenticated acceptance and restores automatic gates.
7. Retire migration-only trust after verified cutover, preserve bounded recovery
   evidence, and remove these workflow artifacts after the implementation PR records
   the completed outcome. Never delete backup keys or snapshots before the rollback
   window closes explicitly.
