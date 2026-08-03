# Smoke Credential Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow the replacement-VPS installer to consume any non-empty existing smoke-account password without weakening validation for other secrets.

**Architecture:** Keep the current centralized `_validate_secret` boundary. Split `SMOKE_PASSWORD` from `NEON_DB_PASSWORD`, assign it a minimum length of one character, and retain the existing unsafe-input and required-input checks.

**Tech Stack:** Bash 4.4+, Bats

## Global Constraints

- `SMOKE_PASSWORD` is an existing opaque credential, not a password created by the installer.
- Newlines and known placeholder markers remain rejected.
- Validation for every other secret remains unchanged.
- Do not commit or print production credentials.

---

### Task 1: Accept an Existing Non-Empty Smoke Password

**Files:**
- Modify: `scripts/tests/installer/config_state.bats`
- Modify: `scripts/lib/schoolorbit-installer/config.sh`

**Interfaces:**
- Consumes: `_validate_secret NAME VALUE` and `load_inputs` from `scripts/lib/schoolorbit-installer/config.sh`.
- Produces: `load_inputs` accepts a non-empty `SMOKE_PASSWORD` shorter than 12 characters while preserving all other validation.

- [ ] **Step 1: Write the failing regression test**

Add this test to `scripts/tests/installer/config_state.bats`:

```bash
@test "accepts a non-empty existing smoke password without imposing creation policy" {
    local input="$TEST_ROOT/short-smoke-password.json"
    jq '.SMOKE_PASSWORD = "short-ok"' \
        "$BATS_TEST_DIRNAME/fixtures/secrets.json" >"$input"
    parse_args migrate-vps --repository owner/repo --target 192.0.2.20 --secrets-stdin

    load_inputs <"$input"

    [ "${SO_SECRETS[SMOKE_PASSWORD]}" = short-ok ]
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
bats --filter 'accepts a non-empty existing smoke password' scripts/tests/installer/config_state.bats
```

Expected: FAIL with `Value for SMOKE_PASSWORD is too short`.

- [ ] **Step 3: Implement the minimal validation change**

Change the relevant `case` arms in `_validate_secret`:

```bash
NEON_DB_PASSWORD)
    minimum=12
    ;;
SMOKE_PASSWORD)
    minimum=1
    ;;
```

Do not modify the preceding `_contains_unsafe_input` call or any other secret rule.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
bats --filter 'accepts a non-empty existing smoke password' scripts/tests/installer/config_state.bats
```

Expected: PASS.

- [ ] **Step 5: Run installer verification**

Run:

```bash
bats scripts/tests/installer
shellcheck scripts/schoolorbit-installer scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
shfmt -d -i 4 -ci scripts/schoolorbit-installer scripts/lib/schoolorbit-installer/*.sh scripts/lib/schoolorbit-installer/remote/*.sh
node --test frontend-school/tests/static/deployment-installer.test.mjs
git diff --check
```

Expected: every command exits successfully.

- [ ] **Step 6: Commit the implementation**

```bash
git add scripts/tests/installer/config_state.bats scripts/lib/schoolorbit-installer/config.sh
git commit -m "fix: accept existing smoke credentials"
```
