# Frontend School Global Density Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every `frontend-school` route render at a consistent 90% presentation density while the browser remains at 100% and viewport-filling layouts retain their real viewport size.

**Architecture:** The root SvelteKit layout already imports one shared stylesheet for every route and body-level portal. Add a single 90% `html` font-size baseline there so Tailwind's rem-based design system scales globally, while explicitly forbidding CSS `zoom` and transform scaling so viewport units, fixed overlays, and responsive breakpoints remain stable.

**Tech Stack:** SvelteKit 5, Svelte 5, TypeScript, Tailwind CSS 4, Node test runner, Playwright Chromium

## Global Constraints

- The 90% baseline applies to every public, authentication, staff, student, parent, embedded-calendar, dialog, toast, and PWA prompt surface.
- `html` uses a 90% font-size baseline in `frontend-school/src/routes/layout.css`.
- Do not introduce CSS `zoom` or transform scaling.
- Do not modify viewport units or responsive breakpoint thresholds.
- Keep browser zoom and user font enlargement functional on top of the application baseline.
- Explicit pixel dimensions remain unchanged unless browser evidence proves that one blocks the global density outcome; any exception must be narrow and separately justified.
- This remains presentation-only: no backend, database, API contract, permission, realtime, security/PDPA, or deployment changes.
- Follow test-driven development: observe the focused regression test failing before changing the stylesheet.

## File Structure

- Modify `frontend-school/tests/static/frontend-layout-components.test.mjs` to own the durable root-layout density contract alongside the existing shared-layout architecture checks.
- Modify `frontend-school/src/routes/layout.css` to own the global 90% rem baseline.
- Do not change a `.svelte` component unless browser verification reveals a concrete incompatibility; if that happens, stop and revise this plan before broadening scope.

---

### Task 1: Lock and implement the global density baseline

**Files:**
- Modify: `frontend-school/tests/static/frontend-layout-components.test.mjs:27`
- Modify: `frontend-school/src/routes/layout.css:144-148`

**Interfaces:**
- Consumes: the existing root-layout side-effect import `import './layout.css';` in `frontend-school/src/routes/+layout.svelte`
- Produces: the global CSS contract `html { font-size: 90%; }` inherited by every route and body-level portal

- [ ] **Step 1: Add the failing static regression test**

Insert this test after `listFiles` and before the existing shared app-layout test:

```js
test('frontend school uses a global 90% density baseline', async () => {
	const rootLayout = await readProjectFile('src/routes/+layout.svelte');
	const layoutStyles = await readProjectFile('src/routes/layout.css');

	assert.match(
		rootLayout,
		/import '\.\/layout\.css';/,
		'the root layout must load the stylesheet that owns global density'
	);
	assert.match(
		layoutStyles,
		/html\s*\{[^}]*\bfont-size:\s*90%;[^}]*\}/,
		'the shared stylesheet must apply the approved 90% root rem baseline'
	);
	assert.doesNotMatch(
		layoutStyles,
		/\bzoom\s*:/,
		'CSS zoom would shrink viewport-sized workspaces below the real viewport'
	);
	assert.doesNotMatch(
		layoutStyles,
		/\btransform\s*:[^;{}]*\bscale\(/,
		'transform scaling would leave layout dimensions and fixed positioning inconsistent'
	);
});
```

- [ ] **Step 2: Run the focused test and confirm the red state**

Run from `frontend-school`:

```bash
node --test \
  --test-name-pattern='frontend school uses a global 90% density baseline' \
  tests/static/frontend-layout-components.test.mjs
```

Expected: FAIL at the `font-size: 90%` assertion with `The input did not match the regular expression`; the import and forbidden-scaling assertions should not be the failing boundary.

- [ ] **Step 3: Add the minimal global stylesheet implementation**

Update the existing base-layer `html` rule in `frontend-school/src/routes/layout.css` to exactly:

```css
html {
	font-size: 90%;
	font-family: 'Kanit', sans-serif;
	-webkit-font-smoothing: antialiased;
	-moz-osx-font-smoothing: grayscale;
}
```

Do not add page-specific classes, CSS `zoom`, transforms, viewport compensation, or breakpoint overrides.

- [ ] **Step 4: Run the focused test and confirm the green state**

Run from `frontend-school`:

```bash
node --test \
  --test-name-pattern='frontend school uses a global 90% density baseline' \
  tests/static/frontend-layout-components.test.mjs
```

Expected: PASS with one selected test passing and the unrelated tests skipped by the name filter.

- [ ] **Step 5: Check formatting for the two changed implementation files**

Run from `frontend-school`:

```bash
npx prettier --check \
  src/routes/layout.css \
  tests/static/frontend-layout-components.test.mjs
```

Expected: PASS with both files reported as correctly formatted. If it fails, run the same command with `--write`, inspect the formatting-only diff, and rerun `--check`.

- [ ] **Step 6: Review and commit the test-first implementation**

Run from the repository root:

```bash
git diff --check
git diff -- frontend-school/src/routes/layout.css \
  frontend-school/tests/static/frontend-layout-components.test.mjs
git status --short
git add frontend-school/src/routes/layout.css \
  frontend-school/tests/static/frontend-layout-components.test.mjs
git commit -m "style(frontend-school): apply global compact density"
```

Expected: the diff contains one CSS declaration and one focused regression test, with no generated, backend, contract, or component changes.

---

### Task 2: Verify representative routes at real desktop and mobile viewports

**Files:**
- Verify only: `frontend-school/src/routes/+page.svelte`
- Verify only: `frontend-school/src/routes/login/+page.svelte`
- Verify only: `frontend-school/src/lib/components/calendar/PublicCalendarView.svelte`
- Temporary screenshots: a directory created by `mktemp -d`; do not add screenshots to Git

**Interfaces:**
- Consumes: the global `html { font-size: 90%; }` contract from Task 1
- Produces: browser evidence that the computed root size is 14.4px at the default 16px browser baseline, full-height route roots still match the physical viewport, and representative routes do not create horizontal overflow

- [ ] **Step 1: Start a local frontend server with non-secret test configuration**

Run from `frontend-school` in a persistent terminal session:

```bash
PUBLIC_BACKEND_URL=http://127.0.0.1:3000 \
PUBLIC_VAPID_KEY=test \
npm run dev -- --host 127.0.0.1 --port 4173
```

Expected: Vite reports `http://127.0.0.1:4173/`. Keep this session running and proceed without waiting an arbitrary amount of time.

- [ ] **Step 2: Create an isolated screenshot directory**

Run in a second terminal:

```bash
density_artifact_dir="$(mktemp -d)"
export DENSITY_ARTIFACT_DIR="$density_artifact_dir"
```

Expected: `DENSITY_ARTIFACT_DIR` names a new empty temporary directory, not the repository, home directory, or workspace root.

- [ ] **Step 3: Run the viewport and overflow browser diagnostic**

Run from `frontend-school` in the second terminal:

```bash
node --input-type=module - <<'NODE'
import assert from 'node:assert/strict';
import path from 'node:path';
import { chromium } from '@playwright/test';

const baseUrl = 'http://127.0.0.1:4173';
const artifactDir = process.env.DENSITY_ARTIFACT_DIR;
assert.ok(artifactDir, 'DENSITY_ARTIFACT_DIR must be set');

const viewports = [
	{ name: 'desktop', width: 1920, height: 1080 },
	{ name: 'mobile', width: 390, height: 844 }
];
const routes = [
	{ name: 'landing', path: '/', root: '.min-h-screen', ready: 'h1' },
	{ name: 'login', path: '/login', root: '.min-h-screen', ready: 'form' },
	{
		name: 'public-calendar',
		path: '/calendar',
		root: 'main',
		ready: 'button[aria-label*="กิจกรรม"]'
	}
];

const browser = await chromium.launch({ headless: true });
try {
	for (const viewport of viewports) {
		const context = await browser.newContext({ viewport });
		await context.route('**/api/auth/me', (route) =>
			route.fulfill({
				status: 401,
				contentType: 'application/json',
				headers: {
					'Access-Control-Allow-Origin': baseUrl,
					'Access-Control-Allow-Credentials': 'true'
				},
				body: JSON.stringify({ success: false, error: 'unauthenticated' })
			})
		);
		await context.route('**/api/public/calendar/events?*', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				headers: {
					'Access-Control-Allow-Origin': baseUrl,
					'Access-Control-Allow-Credentials': 'true'
				},
				body: JSON.stringify({ success: true, data: [] })
			})
		);

		const page = await context.newPage();
		for (const route of routes) {
			await page.goto(`${baseUrl}${route.path}`);
			await page.locator(route.ready).first().waitFor();
			const metrics = await page.evaluate((rootSelector) => {
				const root = document.querySelector(rootSelector);
				if (!(root instanceof HTMLElement)) throw new Error(`Missing ${rootSelector}`);
				const bounds = root.getBoundingClientRect();
				return {
					rootFontSize: getComputedStyle(document.documentElement).fontSize,
					rootHeight: bounds.height,
					viewportHeight: window.innerHeight,
					documentWidth: document.documentElement.scrollWidth,
					viewportWidth: window.innerWidth
				};
			}, route.root);

			assert.equal(metrics.rootFontSize, '14.4px', `${route.name} root density`);
			assert.ok(
				Math.abs(metrics.rootHeight - metrics.viewportHeight) <= 1,
				`${route.name} must fill ${viewport.name} viewport height`
			);
			assert.ok(
				metrics.documentWidth <= metrics.viewportWidth + 1,
				`${route.name} must not overflow ${viewport.name} viewport width`
			);

			await page.screenshot({
				path: path.join(artifactDir, `${route.name}-${viewport.name}.png`),
				fullPage: false
			});
			console.log(`${route.name}/${viewport.name}`, metrics);
		}
		await context.close();
	}
} finally {
	await browser.close();
}
NODE
```

Expected: all six route/viewport combinations print `rootFontSize: '14.4px'`; root and viewport heights differ by at most one pixel; document width never exceeds viewport width by more than one pixel; the command exits successfully.

- [ ] **Step 4: Inspect the six temporary screenshots**

Open each PNG in `DENSITY_ARTIFACT_DIR` with the local image viewer and confirm:

- no unused strip appears on the right or bottom edge;
- landing and login text and controls remain readable at 390×844;
- the public calendar keeps all seven weekday columns inside the viewport;
- desktop spacing is visibly denser without clipping headings, buttons, cards, or calendar cells.

If a screenshot violates one of these exact checks, record the route, viewport, selector, and measured overflow before changing code. Do not introduce a broad page-by-page adjustment; return to root-cause investigation and revise the approved design if the global rem approach is disproved.

- [ ] **Step 5: Remove the non-sensitive temporary screenshots and stop the server**

After inspection, run in the second terminal:

```bash
find "$DENSITY_ARTIFACT_DIR" -maxdepth 1 -type f -name '*.png' -delete
rmdir "$DENSITY_ARTIFACT_DIR"
unset DENSITY_ARTIFACT_DIR
```

Then send `Ctrl-C` to the persistent Vite session and confirm it exits. These screenshots contain public/local pages only and are not committed.

---

### Task 3: Run the frontend verification matrix and review scope

**Files:**
- Verify: all files touched by Task 1
- Do not modify generated permission or API contract artifacts

**Interfaces:**
- Consumes: the committed CSS baseline and static regression test from Task 1
- Produces: repository-wide evidence required by `.rules` and `docs/TESTING.md`

- [ ] **Step 1: Run frontend formatting and lint checks**

Run from `frontend-school`:

```bash
npm run lint
```

Expected: PASS. Do not disable or suppress an existing rule to obtain a green result.

- [ ] **Step 2: Run Svelte and TypeScript validation**

Run from `frontend-school`:

```bash
PUBLIC_BACKEND_URL=http://localhost:3000 \
PUBLIC_VAPID_KEY=test \
npm run check
```

Expected: PASS with zero Svelte errors and zero warnings. No Svelte autofixer call is required because the planned implementation does not touch a `.svelte`, `.svelte.ts`, or `.svelte.js` file.

- [ ] **Step 3: Run the frontend runtime and static suites**

Run from `frontend-school`:

```bash
npm run test:menu-sync
npm run test:static
```

Expected: both commands PASS. The global density regression must pass as part of `test:static`.

- [ ] **Step 4: Review the final repository state**

Run from the repository root:

```bash
git diff --check
git status --short
git log -3 --oneline
```

Expected: `git diff --check` passes; the only implementation commit after this plan is `style(frontend-school): apply global compact density`; no migration, generated contract, backend, secret, screenshot, or unrelated user file is present.

- [ ] **Step 5: Record verification limits accurately**

In the handoff, list every exact command above with pass/fail status. State that the unauthenticated local browser diagnostic covered landing, login, and public calendar routes at 1920×1080 and 390×844. Do not claim authenticated staff/student/parent workflow screenshots were run unless dedicated runtime credentials were actually supplied; their adoption of the global density contract is established structurally by the root layout import and focused regression test.
