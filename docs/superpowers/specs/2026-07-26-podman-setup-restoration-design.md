# Podman Setup Guide Restoration Design

## Context

The documentation consolidation removed `docs/PODMAN_SETUP.md`, while `podman-compose.yml`, Nginx configuration, and Podman-based deployment workflows remain active. The user wants the installation guide restored as a separate permanent document rather than merged into `docs/OPERATIONS.md`.

The removed guide cannot be restored verbatim because it duplicated large Compose/Nginx configurations, included unsafe Cockpit root-login advice, and could drift from the executable files.

## Goal

Create a current Thai-language `docs/PODMAN_SETUP.md` that helps an operator install and bootstrap SchoolOrbit on a Debian/Ubuntu server using Podman without duplicating executable configuration.

## Documentation Structure

The guide will cover:

1. supported topology and prerequisites;
2. installing Podman, podman-compose, Cockpit, Git, curl, and certificate tooling;
3. creating and securing `/opt/stack`;
4. obtaining the repository files used by the stack;
5. creating `/opt/stack/.env` from confirmed environment names without real secret values;
6. starting backend services from `podman-compose.yml`;
7. attaching the Nginx container to the same Podman network and using repository Nginx references;
8. configuring DNS and TLS without prescribing provider-console secrets;
9. verifying `/health`, `/ready`, containers, networks, and logs;
10. explaining the current GitHub deployment workflow assumptions;
11. safe update, rollback, and troubleshooting commands.

The guide will link to `podman-compose.yml`, current Nginx files, `docs/OPERATIONS.md`, and `docs/TESTING.md` instead of embedding full copies.

## Safety Constraints

- Do not recommend Cockpit root login. Use a normal administrative user with `sudo`.
- Do not place real secrets in commands, source files, screenshots, or logs.
- Do not invent environment variables; derive names from current Compose files and examples.
- Do not expose database or application ports publicly unless the deployment requires it and firewall policy is reviewed.
- Use `/ready` for deployment gating and `/health` for liveness.
- Preserve stable `ENCRYPTION_KEY` and `BLIND_INDEX_KEY`; rotation remains a separate reviewed operation.
- Treat example defaults as local-only and require production secrets in `/opt/stack/.env`.

## Canonical Documentation Policy

`docs/PODMAN_SETUP.md` becomes the explicitly approved twelfth permanent Markdown file.

The same change will:

- add it to `docs/README.md`;
- link it from `docs/OPERATIONS.md`;
- update `.rules` from an 11-file to a 12-file documentation set;
- add it to `frontend-school/tests/static/documentation-policy.test.mjs`;
- update the documentation policy test name where it improves clarity.

This temporary design document will be deleted before final verification and will not be added to the permanent allowlist.

## Verification

Use a red-green policy cycle:

1. update the documentation allowlist first and confirm the check fails because `docs/PODMAN_SETUP.md` is missing;
2. add the guide and canonical links;
3. remove this temporary spec;
4. run `npm run check:docs`;
5. run the affected frontend static suite;
6. verify exactly 12 tracked Markdown files remain;
7. run `git diff --check` and confirm a clean final worktree after commit.

Environment-dependent deployment commands will be documented but not executed against a live server as part of this documentation-only change.
