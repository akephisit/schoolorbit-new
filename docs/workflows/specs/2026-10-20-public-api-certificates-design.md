# Public API certificates

Approved scope: give `school-api.schoolorbit.app` and `admin-api.schoolorbit.app`
publicly trusted HTTPS, verify both directly, then change only their Cloudflare A
records to DNS only. Preserve API names, full timetable responses, application
versions, frontend Workers, apex/wildcard DNS and the management Tunnel.

## Ownership and interface

`scripts/lib/pipeline-remote/api-tls.sh` owns ACME account state, DNS-01 issuance,
validation, staged generations and activation. The installer and serialized
maintenance workflow call that same owner. Use the official Certbot DNS Cloudflare
5.8.0 container pinned by digest. Rootless Podman already owns the production
runtime; no privileged package installation or independent host renewal timer is
needed. Certbot state and private logs remain under `/opt/stack/acme` with private
permissions. A credential arrives through protected stdin or SSH environment and
exists only in a temporary mode-0600 credentials file. Never persist a deployment
token in application environment, checkpoints or certificate state.

Keep the Nginx `schoolorbit-origin.pem` and `schoolorbit-origin.key` interfaces.
They identify this origin's certificate, regardless of issuer. Export Certbot's
renewed material into a separate immutable generation so renewal cannot change a
serving certificate before maintenance. Validate the public chain, exact two SANs,
key match and remaining validity before promotion. After the initial migration,
stable certificate/key links share one atomically replaced generation link.

Normal deployment, installer verification and API smoke use system CA trust with
hostname-pinned destination routing. There is no Origin CA fallback. Local test
fixtures may explicitly trust their synthetic CA.

## Bootstrap, renewal and migration

DNS-01 works before a replacement VPS receives traffic; HTTP-01 cannot validate a
new destination while public DNS reaches the old origin. The installer issues and
activates the public certificate before its first Pipeline, then verifies the
target directly before its existing confirmed DNS cutover. Its checkpoint stores
public certificate fingerprint and expiry rather than a Cloudflare certificate ID.

The existing Maintenance workflow owns daily renewal under the existing
`schoolorbit-production` lock, with cancellation disabled. Retention retains its
weekly schedule. A due renewal stages material first, enters universal maintenance,
activates and validates both direct APIs plus authenticated academic smoke, then
reopens the same accepted application release. It never publishes application
artifact baselines. No changed certificate means no maintenance or reload.

The Senior temporarily disables automatic rollout gates before integrating the
first change. The exact main SHA must pass its Pipeline planning/build gates before
the explicit migration operation runs under the production lock. That operation
stages the public certificate, enters maintenance using an explicit, hash-pinned
Origin CA transition mode, then activates the public certificate. Every subsequent
check uses public trust. No normal release or renewal can silently select old trust.
The bounded transition is retired after successful live cutover. The normal
prepared Pipeline then reconciles queued changes before automatic gates reopen.

DNS cutover snapshots both exact API A records and rejects ambiguity, extra AAAA or
CNAME records and drift. Direct public-trust readiness and authenticated full
academic-context smoke must pass before changing `proxied` to false. Preserve every
unrelated record. Rollback restores the original two records, including TTL and
proxy flags; rollback requires a fresh match to the journaled cutover state.

## Failure and acceptance

Invalid chains, names, keys, permissions or expiry never activate. Failed Nginx
validation/reload restores the former certificate generation; failures after
maintenance leave maintenance active for controlled recovery. Retain the previous
certificate and DNS snapshot through the explicitly closed rollback window.

Acceptance requires first-install/resume fixtures, renewal/no-op/failure recovery,
credential cleanup, migration trust isolation, DNS-only cutover/drift/rollback,
normal deployment checks and direct HTTPS without custom CA flags. Run the
documentation and deployment verification owners, independent review and QA, and
live authenticated smoke. No application, database, API contract or payload change
is part of this design.
