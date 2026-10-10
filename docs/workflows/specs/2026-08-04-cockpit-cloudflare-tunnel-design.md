# Cockpit over Cloudflare Tunnel Design

## Context

SchoolOrbit runs its production containers as rootless Podman owned by the `schoolorbit`
service user. Operators want the browser-based Cockpit experience used on the previous VPS,
including the Podman page, without exposing TCP port 9090 on the replacement VPS. The VPS
installer must make the same management surface available on future Debian and Ubuntu targets.

The accepted request path is:

```text
operator browser
    -> Cloudflare edge
    -> Cloudflare Tunnel
    -> Cockpit on 127.0.0.1:9090
    -> rootless Podman owned by schoolorbit
```

This is a public Cloudflare Tunnel application, not a Cloudflare Access application. The public
hostname displays Cockpit's normal login page directly.

## Goals

- Publish Cockpit at `https://server.${base_domain}` through an outbound-only Cloudflare Tunnel.
- Keep Cockpit bound to loopback and keep port 9090 closed to public ingress.
- Authenticate through Cockpit with the `schoolorbit` Linux account so its rootless containers
  are visible.
- Keep direct `root` login disabled. Host-level administration remains a separate, explicitly
  authorized workflow.
- Add one idempotent management-surface implementation that is called by `migrate-vps` and can
  also be applied to an already migrated VPS.
- Keep Cloudflare and Linux credentials out of command arguments, logs, checkpoints, and Git.
- Preserve the previous management route and tunnel during the rollback window.

## Non-goals

- Cloudflare Access, OTP, identity-provider, or account-member authentication.
- Public inbound access to port 9090.
- Rootful Podman or moving SchoolOrbit containers from the `schoolorbit` user to `root`.
- Automatic deletion of the old VPS, its Tunnel, or its Cockpit configuration.
- General-purpose VPS administration through the application deployment workflows.

## Security Boundary

Cloudflare Tunnel hides the origin port and carries requests over outbound connections, but it
does not make the published hostname private. Anyone can reach the Cockpit login page. The
operator must therefore use a unique, strong `schoolorbit` password. The installer accepts that
password only from an environment variable, hidden prompt, or the existing `--secrets-stdin`
object, requires at least 10 characters, streams it to the privileged bootstrap script, and never
writes it to the checkpoint or GitHub. Ten characters is the operator-approved compatibility
floor; uniqueness and greater length remain recommended because this login page is public.

Cockpit keeps `root` in `/etc/cockpit/disallowed-users`. This is both a security boundary and a
runtime ownership boundary: a root Cockpit session would inspect root's Podman namespace rather
than the production containers owned by `schoolorbit`.

Cockpit listens only on `127.0.0.1:9090`. Its reverse-proxy configuration accepts only
`https://server.${base_domain}` as an origin and trusts the forwarded protocol and client address
headers only because the listener is unreachable except from the local host. Cloudflare Tunnel
routes to `http://127.0.0.1:9090`; browser-to-Cloudflare and Cloudflare-to-connector traffic remain
encrypted, while the final HTTP hop stays on loopback.

The Cloudflare bootstrap token retains its existing permissions and adds only `Cloudflare Tunnel
Write`; its existing zone-scoped `DNS Write` permission owns the management CNAME. Tunnel runtime
credentials are written atomically to a root-owned mode-0600 file and consumed by the
`cloudflared` system service without appearing in command-line arguments.

## Components and Ownership

### Installer configuration

The installer adds one required secret for Cockpit-enabled operations, named for the
`schoolorbit` account rather than for `root`. It also derives the management hostname from the
existing base domain. Checkpointed configuration records only the hostname, tunnel identifier,
DNS record metadata, and verification codes.

`migrate-vps --dry-run` verifies Cloudflare Tunnel read/write authorization, the management DNS
shape, target OS support, and required local tools without creating a Tunnel, changing DNS,
installing packages, or changing a password.

### Cloudflare provider

A focused provider module owns management Tunnel operations instead of extending the API-origin
DNS functions with unrelated conditionals. It:

1. requires zero or one `server.${base_domain}` CNAME and rejects ambiguous A/AAAA/CNAME records;
2. snapshots an existing management record and its referenced Tunnel without storing a runtime
   token;
3. creates or reuses only a Tunnel whose checkpointed identity belongs to the current installer
   run;
4. configures one ingress rule for the management hostname plus a final `http_status:404`
   catch-all;
5. obtains the Tunnel runtime token only in memory for installation;
6. waits for at least one healthy connector before publishing or replacing the CNAME; and
7. verifies that the public hostname reaches Cockpit through Cloudflare.

Each replacement VPS receives a distinct Tunnel. A future migration must not attach the new VPS
as a replica of the old Cockpit Tunnel because Cloudflare could then send an operator to either
host. The stable management CNAME moves only after the new connector and local Cockpit pass their
checks.

### VPS bootstrap

The privileged, idempotent bootstrap installs `cockpit`, `cockpit-podman`, and a pinned
`cloudflared` package in addition to the existing runtime packages. It:

- preserves the existing `schoolorbit` user and linger configuration;
- starts that user's systemd manager and enables `podman.socket` in the `schoolorbit` user
  session so Cockpit Podman reaches `/run/user/$(id -u schoolorbit)/podman/podman.sock`;
- keeps the production containers in the existing rootless `schoolorbit` namespace and does not
  restart, recreate, or copy them into root's Podman storage;
- updates that user's password through stdin without echoing it;
- installs Cockpit's origin and proxy-header configuration atomically;
- overrides `cockpit.socket` to listen only on `127.0.0.1:9090`;
- keeps `root` disallowed;
- installs the Tunnel credential atomically with mode 0600;
- enables and starts `cockpit.socket` and `cloudflared.service`; and
- verifies service state from a fresh SSH session.

The implementation must be safe to run repeatedly. Unchanged files and services are left alone,
and a failed validation must not replace the last working configuration.

### Orchestration

The management setup is a separately testable installer phase that uses the same implementation
from two entry paths:

- `migrate-vps` provisions and verifies the management surface after the replacement APIs pass
  direct-origin verification. It switches the stable management CNAME only after public
  application verification succeeds.
- an explicit management setup command applies the phase to an existing target without
  redeploying applications, rerunning tenant migrations, or changing the two API DNS records.

The current VPS at `130.94.21.134` uses the explicit path after the implementation has passed the
local and CI checks.

## Failure and Rollback Behavior

- Failure before CNAME publication leaves the existing management hostname untouched.
- Failure after CNAME publication reports the exact management rollback command and retains both
  Tunnels for diagnosis.
- A confirmed DNS rollback restores the previous management CNAME or removes the new record when
  no previous record existed. It does not delete either Tunnel or VPS.
- Re-running the same checkpoint revalidates completed work and resumes from the first incomplete
  management step.
- A missing or unhealthy Tunnel, an inactive `schoolorbit` Podman API socket, a non-loopback
  Cockpit listener, an unexpected DNS record type, an origin mismatch, or a leaked secret is a
  hard failure.
- Management-surface failure never silently triggers application DNS rollback. The operator
  chooses management rollback separately because the APIs and frontends may already be healthy.

## Verification

Tests cover:

- Cloudflare permission preflight and zero-or-one management record validation;
- distinct Tunnel creation, ingress catch-all, connector readiness, CNAME publication, resume,
  drift rejection, and confirmed rollback;
- absence of Tunnel tokens and the `schoolorbit` password from logs, command arguments, state,
  and runtime application environment;
- consistent password validation at both installer and remote boundaries, rejecting 9 characters
  and accepting 10 or more;
- Debian and Ubuntu package/bootstrap idempotency;
- idempotent enablement of the `schoolorbit` user manager and Podman API socket, including a fresh
  check that the socket is active and exists at the expected per-user runtime path;
- loopback-only Cockpit socket configuration, proxy origin/header configuration, root-login
  prohibition, and service enablement;
- local Cockpit `/ping`, public login-page response, and WebSocket-compatible proxying;
- the existing ShellCheck, shfmt, Bats, deployment static guard, Compose dry-run, and actionlint
  matrix.

Live acceptance on the current VPS additionally requires:

1. no public listener on port 9090;
2. a healthy Tunnel connector whose origin IP is the replacement VPS;
3. `https://server.schoolorbit.app` displays Cockpit directly without a Cloudflare Access page;
4. `/run/user/$(id -u schoolorbit)/podman/podman.sock` is active and its Libpod API lists the four
   SchoolOrbit rootless containers;
5. login as `schoolorbit` shows those same four containers in Cockpit Podman; and
6. both public APIs and tenant frontends remain healthy and unchanged.

## Residual Risk

The Cockpit login page is public by explicit operator choice. Cloudflare provides the Tunnel,
proxy, WAF, and DDoS boundary, while Cockpit and the Linux password remain the only user
authentication layer. This is less restrictive than Cloudflare Access. If exposure becomes
unacceptable, Access can be added later without changing the loopback listener or rootless
Podman ownership.
