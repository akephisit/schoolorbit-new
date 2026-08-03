# Smoke Credential Validation Design

## Context

The replacement-VPS installer consumes an existing SchoolOrbit account only to run authenticated post-cutover smoke checks. It currently requires `SMOKE_PASSWORD` to contain at least 12 characters even when the application already accepts the supplied account credential. This prevents recovery while the old reverse proxy is unavailable and the account password cannot be changed through the UI.

## Decision

Treat `SMOKE_PASSWORD` as an existing opaque credential rather than a newly created password. The installer will require it to be non-empty and will retain the shared unsafe-input checks for newlines and known placeholder markers. All other secret length, format, and safety validation remains unchanged.

This change does not modify SchoolOrbit authentication, password creation, password hashing, or application password policy. It changes only workstation-installer input validation.

## Verification

- Add a regression test proving that a non-empty short smoke password is accepted with an otherwise valid installer input document.
- Keep existing coverage proving that missing required inputs and unsafe placeholder values are rejected.
- Run the focused installer Bats suite, shell lint/format checks for touched shell code, static installer tests, and `git diff --check`.

## Operational Follow-up

The existing smoke account can be used to restore service. Because its current password has been disclosed, change it after the new runtime is healthy and update the GitHub smoke secret at the same time.
