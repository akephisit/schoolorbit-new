# Release test infrastructure repair plan

> Use executing-plans inline, one task at a time. Do not delegate. The user subsequently authorized the migration-057 test repair and committing/pushing the verified changes to main.

**Goal:** Remove the two test-infrastructure blockers to complete backend-school release verification.

**Architecture:** Give the existing disposable local PostgreSQL runner an anonymous disk-backed volume, removed with its exact container. Bring the learning-offering runtime-policy fixture through the existing current migrations without changing policy assertions or migration files.

**Tech stack:** Bash, Docker, Node test runner, Rust/SQLx, PostgreSQL.

**Spec:** User-approved repair of the 5 GiB test database exhaustion and stale migration-53 policy fixture identified during LTO verification.

## Constraints

Keep local-endpoint validation, loopback port binding, argument forwarding, failure status, and signal cleanup. Never reuse a named volume, prune unrelated resources, edit applied migrations, change runtime authorization, or skip failing tests. Preserve the earlier LTO change and original extraction worktree.

## 1. Disposable storage

- [x] Change the existing runner tests to require `--mount type=volume,destination=/var/lib/postgresql` and `docker rm --force --volumes <exact-container>` on success and failure. Run `node --test --test-concurrency=1 scripts/tests/backend-school-test-database.test.mjs` and observe red.
- [x] Remove `POSTGRES_TMPFS_SIZE`; replace the data tmpfs with the anonymous mount and add `--volumes` to the existing cleanup. Keep shared-memory settings and error handling. Re-run the Node suite, `shellcheck scripts/test_backend_school.sh`, and `shfmt -d -i 4 -ci scripts/test_backend_school.sh` (containerized tools if unavailable on the host).
- [x] Document disk capacity and exact-volume cleanup in `docs/TESTING.md`; verify the real runner on a small test and inspect its owned mount and cleanup.

## 2. Runtime policy fixture

- [x] Reproduce `policies::learning_offering_access_policy::tests::offering_policy_unions_assignment_unit_and_tree_without_expanding_school_access` on a fresh disposable database before editing.
- [x] Replace the fixture's last migration-53 cutoff with `run_test_migrations(&pool).await` from `crate::test_helpers`. The cutoff helper requires an existing version and does not accept a sentinel. This is a current-runtime test, not a historical migration test. Keep all access assertions unchanged.
- [x] Align fixture ownership with migration 59: use activity/math units for the actor's subtree and a science course assigned to another teacher outside it. Seed the complete science curriculum/course predecessor through `seed_learning_offering_policy_science_course` in the existing test-only `cutover_test_support.rs`; historical SQL must not enter the runtime policy file.
- [x] Re-run the focused test and then the complete release suite with Rust 1.98.0 and the existing final-crate-only LTO wrapper, one test thread. Inspect database volume usage and verify removal after completion. After relocating the unchanged seed SQL into test support, repeat the focused release test and architecture checks.

## 3. Verification and handoff

- [x] Run backend formatting, static architecture tests, and Cargo check sequentially. Run documentation guards and runner tests, inspect the final diff, and run `git diff --check` and `git status --short`.
- [x] Report full-suite and post-relocation focused results separately, plus any remaining production-only gates. Do not claim deployment acceptance from local test results.

## 4. Pre-push migration-057 test repair

The fresh pre-push release run exposed a random-UUID-dependent assertion: it selected the first midterm control, which could be the newly backfilled default-false row rather than an existing true row.

- [x] Capture an existing control's term ID and use it for both the pre-migration update and post-migration assertion. Give the missing term a deterministic low UUID and separately assert its default-false value. Do not modify migration 057.
- [x] Run the focused migration test, full release suite, and applicable verification checks sequentially; review the scoped diff.
- [ ] Commit the verified changes, fast-forward main, and push without force. Preserve the original extraction worktree and distinguish push success from deployment success.
