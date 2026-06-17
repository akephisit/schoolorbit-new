# Frontend Loading UX Standardization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Standardize loading, empty, error, and permission-blocked states across frontend-school staff modules using local shadcn-svelte primitives.

**Architecture:** Add small shared state components under `frontend-school/src/lib/components/app-state/` that compose local shadcn-svelte UI primitives. Migrate pages module-by-module so initial page loading uses layout-stable skeletons, action loading uses a consistent button spinner, and read-first permission UX stays intact.

**Tech Stack:** SvelteKit 5, TypeScript, Tailwind CSS, local shadcn-svelte components, lucide-svelte, node:test static guards.

---

### Task 1: Shared App State Components

**Files:**
- Create: `frontend-school/src/lib/components/ui/skeleton/skeleton.svelte`
- Create: `frontend-school/src/lib/components/ui/skeleton/index.ts`
- Create: `frontend-school/src/lib/components/app-state/PageState.svelte`
- Create: `frontend-school/src/lib/components/app-state/PageSkeleton.svelte`
- Create: `frontend-school/src/lib/components/app-state/TableSkeleton.svelte`
- Create: `frontend-school/src/lib/components/app-state/LoadingButton.svelte`
- Create: `frontend-school/src/lib/components/app-state/index.ts`
- Test: `frontend-school/tests/static/frontend-state-components.test.mjs`

- [ ] **Step 1: Write the failing static test**

Add a node:test file that asserts:
- app-state components exist and import local shadcn-svelte `Alert`, `Button`, `Card`, and `Table`.
- `Skeleton` is exported from `src/lib/components/ui/skeleton`.
- pages migrated later import from `$lib/components/app-state`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd frontend-school && npm run test:static -- tests/static/frontend-state-components.test.mjs`

Expected: FAIL because the shared components do not exist yet.

- [ ] **Step 3: Implement shared components**

Create:
- `Skeleton`: small shadcn-compatible primitive with `data-slot="skeleton"` and `animate-pulse rounded-md bg-muted`.
- `PageState`: reusable empty/error/permission state based on `Alert` or `Card`, with optional action snippet.
- `PageSkeleton`: page-level skeleton variants for `table`, `cards`, `form`, and `detail`.
- `TableSkeleton`: table header/body skeleton using local `Table` primitives.
- `LoadingButton`: local `Button` wrapper with `LoaderCircle` and stable disabled/loading behavior.

- [ ] **Step 4: Run focused verification**

Run:
```bash
cd frontend-school
npm run test:static -- tests/static/frontend-state-components.test.mjs
npm run check
```

Expected: both commands exit 0.

- [ ] **Step 5: Commit**

```bash
git add frontend-school/src/lib/components/ui/skeleton frontend-school/src/lib/components/app-state frontend-school/tests/static/frontend-state-components.test.mjs
git commit -m "feat: add shared frontend state components"
git push origin main
```

### Task 2: Staff Management And Student Workspaces

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/manage/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/students/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/manage/[id]/+page.svelte`
- Modify: `frontend-school/src/routes/(app)/staff/students/[id]/+page.svelte`

- [ ] **Step 1: Write static guard**

Extend `frontend-state-components.test.mjs` so these pages must import from `$lib/components/app-state`.

- [ ] **Step 2: Verify red**

Run: `cd frontend-school && npm run test:static -- tests/static/frontend-state-components.test.mjs`

Expected: FAIL until the listed pages import and use the shared state components.

- [ ] **Step 3: Migrate pages**

Use:
- `PageSkeleton` for initial page/table loading.
- `PageState` for empty, error, and no-read-permission states.
- `LoadingButton` for delete/save actions where local action loading is shown in buttons.

- [ ] **Step 4: Verify and commit**

Run:
```bash
cd frontend-school
npm run test:static -- tests/static/frontend-state-components.test.mjs
npm run check
```

Commit:
```bash
git add frontend-school/src/routes/'(app)'/staff/manage/+page.svelte frontend-school/src/routes/'(app)'/staff/students/+page.svelte frontend-school/src/routes/'(app)'/staff/manage/'[id]'/+page.svelte frontend-school/src/routes/'(app)'/staff/students/'[id]'/+page.svelte frontend-school/tests/static/frontend-state-components.test.mjs
git commit -m "feat: standardize staff and student loading states"
git push origin main
```

### Task 3: Academic Modules

**Files:**
- Modify academic staff pages under `frontend-school/src/routes/(app)/staff/academic/**/+page.svelte`

- [ ] **Step 1: Extend static guard for academic pages**
- [ ] **Step 2: Verify red**
- [ ] **Step 3: Migrate loading, empty, error, and permission-blocked states to shared components**
- [ ] **Step 4: Run `npm run test:static`, `npm run check`, then commit and push**

Commit message: `feat: standardize academic loading states`

### Task 4: Settings, Roles, Organization, Facility, And Menu Modules

**Files:**
- Modify staff settings, roles, organization, facility, menu, features, achievements, work pages under `frontend-school/src/routes/(app)/staff/**/+page.svelte`

- [ ] **Step 1: Extend static guard for the selected module group**
- [ ] **Step 2: Verify red**
- [ ] **Step 3: Migrate state UI to shared components**
- [ ] **Step 4: Run `npm run test:static`, `npm run check`, then commit and push**

Commit message: `feat: standardize staff workspace loading states`

### Task 5: Final Verification

- [ ] **Step 1: Run full frontend verification**

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

- [ ] **Step 2: Run repository diff checks**

```bash
git diff --check
git status --short --branch
```

- [ ] **Step 3: Final push status**

Confirm `main...origin/main` is clean and the latest commit is on `origin/main`.
