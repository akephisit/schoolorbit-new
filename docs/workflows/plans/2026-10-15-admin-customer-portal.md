# Admin customer portal implementation

Design: [approved scope](../specs/2026-10-15-admin-customer-portal-design.md).

1. Add the forward account replacement/preservation gate and independent account,
   session, email-token, throttle, membership, service-period and request owners.
2. Implement Admin-owned email/session/Resend services and typed HTTP contracts.
   Replace legacy authentication and enforce operator/customer boundaries.
3. Add verified-customer school reads/service requests and operator assignment,
   service-period management and request decisions. Expiry is informational only.
4. Update the Admin API/auth store, public auth pages, customer/operator screens
   and local theme. Protect server-side migration forwarding before credentials.
5. Route the Worker to the apex, redirect the old host, align acceptance/installer
   origins and deploy runtime configuration through existing owners.
6. Run focused Rust and disposable DB tests, Admin lint/check/unit/build, browser
   workflows, deployment and documentation policy checks, diff/status review.
7. Open a PR with actual evidence. Keep unverified deployment/prerequisite work
   visible and do not auto-merge an unverified production cutover.
