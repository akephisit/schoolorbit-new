# Auth Checking UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Present one consistent branded authentication-checking state without duplicate portal-level full-screen guards.

**Architecture:** Keep `/api/auth/me` and route authorization in the protected `(app)` layout. Reuse a shared `AuthCheckingState.svelte` component in `(app)` and login, and reduce portal layouts to content wrappers so they cannot display a second checking screen.

**Tech Stack:** SvelteKit 5, Svelte 5 runes, TypeScript, Tailwind CSS, Node test runner

## Global Constraints

- `/api/auth/me` remains the only current-user permission source.
- Frontend route guards remain convenience UX; backend authorization remains authoritative.
- Generated permission and API contracts are unchanged.
- Do not add dependencies or modify migrations.
- Use the existing SchoolOrbit academic icon and theme tokens.

---

### Task 1: Add the regression contract

**Files:**
- Modify: `frontend-school/tests/static/frontend-csr-contract.test.mjs`

**Interfaces:**
- Consumes: Protected, login, and portal layout source files.
- Produces: An architecture test that fails while duplicate auth loading boundaries exist.

- [ ] **Step 1: Write the failing test**

Add a test that reads `src/routes/(app)/+layout.svelte`, `src/routes/login/+page.svelte`, and the three portal layouts. Assert that the first two reference `AuthCheckingState`; assert that portal layouts do not reference `authStore`, `goto`, or the Thai checking copy.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `node --test tests/static/frontend-csr-contract.test.mjs`

Expected: FAIL because `AuthCheckingState` is not yet consumed and portal layouts still own auth/loading logic.

### Task 2: Introduce the shared checking state

**Files:**
- Create: `frontend-school/src/lib/components/app-state/AuthCheckingState.svelte`
- Modify: `frontend-school/src/lib/components/app-state/index.ts`
- Modify: `frontend-school/src/routes/(app)/+layout.svelte`
- Modify: `frontend-school/src/routes/login/+page.svelte`

**Interfaces:**
- Consumes: `message: string` prop and existing theme tokens.
- Produces: `AuthCheckingState` component exported from `$lib/components/app-state`.

- [ ] **Step 1: Implement the minimal shared component**

Create a full-screen, centered state with `GraduationCap`, `aria-live="polite"`, `role="status"`, and the provided message.

- [ ] **Step 2: Replace both inline loading presentations**

Import `AuthCheckingState` from `$lib/components/app-state` in `(app)` and login. Pass the existing checking/redirect messages and remove copied spinner/icon markup.

### Task 3: Remove portal-level auth loading boundaries

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/+layout.svelte`
- Modify: `frontend-school/src/routes/(app)/student/+layout.svelte`
- Modify: `frontend-school/src/routes/(app)/parent/+layout.svelte`

**Interfaces:**
- Consumes: `children` snippets after the parent `(app)` layout has authorized the route.
- Produces: Presentation-only portal wrappers with no auth store or redirect ownership.

- [ ] **Step 1: Simplify the staff layout**

Remove auth imports, state, effects, redirects, and checking markup; render its child snippet directly.

- [ ] **Step 2: Simplify the student and parent layouts**

Remove auth imports, state, effects, redirects, and checking markup; retain their existing `min-h-screen bg-background` content wrapper.

- [ ] **Step 3: Run the focused test to verify GREEN**

Run: `node --test tests/static/frontend-csr-contract.test.mjs`

Expected: PASS with zero failed tests.

### Task 4: Validate Svelte and the frontend matrix

**Files:**
- Validate all Svelte files changed in Tasks 2 and 3.

**Interfaces:**
- Consumes: Completed implementation.
- Produces: Verification evidence required by `.rules`.

- [ ] **Step 1: Run Svelte autofixer**

Run `npx @sveltejs/mcp svelte-autofixer <file> --svelte-version 5` for the new component and each changed `.svelte` file. Resolve relevant issues.

- [ ] **Step 2: Run frontend checks**

Run from `frontend-school`:

```bash
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

- [ ] **Step 3: Inspect the final change**

Run from the repository root:

```bash
git diff --check
git diff --stat
git diff
git status --short
```

Confirm the diff changes only the approved auth-checking UI, portal layout ownership, regression test, and workflow artifacts.
