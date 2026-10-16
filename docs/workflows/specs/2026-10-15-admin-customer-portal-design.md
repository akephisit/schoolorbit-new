# Admin customer portal

Approved in chat: use the apex domain for the public site and customer accounts,
email/password with Resend verification, the existing homepage visual language,
monthly/yearly service periods, and warning-only expiry. Manual operator approval
is the initial service workflow; payment collection is deferred.

## Ownership and preservation

Admin owns its typed DTOs, services, HTTP adapters and frontend API/state/UI.
School authentication, tenant schemas and business data are unchanged. New
accounts, school memberships, service periods and requests live in the Admin DB.
Memberships reference existing school UUIDs without changing school records.
Only operators can assign an existing school to a verified customer.
Customers cannot provision, deploy, migrate, delete or claim arbitrary schools.

Migration 010 replaces the legacy central accounts under the user's explicit
authorization. It reconciles unchanged schools and deployment history in the same
transaction before removing only legacy central accounts. Applied migrations are
immutable. No tenant database, DNS record, encrypted credential or school user is
deleted. New roles have no delete-school HTTP route in this release. The normal
Pipeline validates temporary operator Actions secrets before maintenance, then
runs the canonical account migration and operator bootstrap under the production
lock. Bootstrap creates an unverified account and sends a Resend link. Initial
release may proceed with that pending operator and an active verification link;
login still requires verification through the public page after deployment.
Retries preserve the account/password and never promote a customer. Subsequent
releases skip bootstrap once a verified operator exists.

## Authentication and delivery

Opaque random sessions replace the legacy JWT cookie. Persist only token hashes;
use backend-host-only HttpOnly/Secure/SameSite=Lax cookies, exact configured Origin
checks and a session-derived CSRF header on mutations. Every authenticated read
checks the current account, verification and session in the database. Logout and
password reset revoke sessions. Old JWTs are never accepted. Public account routes
have durable email/source throttles and bounded inputs. Tokens are purpose-bound,
single-use, hashed at rest and expiring. Email URLs carry tokens in fragments and
require an explicit POST so link scanners do not consume them. Resend delivery is
bounded and reports safe recoverable errors; resend recovers an undelivered link.

## Cutover and recovery

The existing Pipeline remains the only production writer. Cloudflare Git Builds
must be disconnected operationally. Serve the apex and former Admin host through
the same Worker; redirect the old host before rendering or maintenance. Update
acceptance and installer origins. Domain and email settings come from environment.
If the Admin Worker is absent, Actions creates an isolated, credential-free 503
bootstrap with routes and preview hosts disabled before the normal version upload
and exact promotion. Existing Workers and all School resources are preserved.
No production reset or promotion is executed from this workspace. Required
Resend sender-domain verification, protected bootstrap Actions secrets and a
fresh recovery point are first-release prerequisites. The user does not pull
images or run production migrations manually.

Schema 010 and the new account runtime must deploy together under maintenance.
After account replacement, old Admin binaries are prohibited. Recovery rolls
forward with the retained school data or uses the existing reviewed backup restore
procedure. Never restore an old account binary against schema 010.

## Acceptance

Disposable DB fixtures prove preservation, repeated migration safety, session
revocation, token expiry/replay, customer/operator separation, scoped school reads,
and warning-only expiry. Frontend lint, type, unit, build and browser checks cover
registration, verification, login, reset, customer and operator workflows. Run the
deployment suite and denied-origin/auth proxy smoke; unavailable production
credentials are reported explicitly and block ready delivery.
