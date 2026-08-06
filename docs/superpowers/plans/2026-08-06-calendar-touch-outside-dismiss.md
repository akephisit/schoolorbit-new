# Calendar Touch Outside Dismiss Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the WordPress-embedded calendar's mobile day dialog close when a visitor taps the dimmed overlay.

**Architecture:** Extend the local `Dialog.Content` wrapper with typed overlay property forwarding, then opt only `CalendarDayTimelineDialog` into an overlay click callback that updates its bound open state. Protect the touch-specific behavior with a Playwright regression test running under an iPhone device profile.

**Tech Stack:** Svelte 5, TypeScript, Bits UI dialog primitives, Playwright, Node.js test runner

## Global Constraints

- Change only frontend dialog interaction; do not change backend behavior, migrations, permissions, generated API contracts, authentication, CSP, iframe sandboxing, or calendar visibility rules.
- Preserve existing close-button, Escape-key, mouse dismissal, and inside-dialog interaction behavior.
- Do not change overlay behavior for dialogs other than `CalendarDayTimelineDialog`.
- Use the generated calendar API contracts without modification.

---

### Task 1: Add touch-overlay dismissal to the calendar day dialog

**Files:**
- Create: `frontend-school/tests/e2e/calendar-embed-dialog.spec.ts`
- Modify: `frontend-school/src/lib/components/ui/dialog/dialog-content.svelte`
- Modify: `frontend-school/src/lib/components/calendar/CalendarDayTimelineDialog.svelte`

**Interfaces:**
- Consumes: `Dialog.Overlay`, `DialogPrimitive.ContentProps`, and the existing bindable `open: boolean` property of `CalendarDayTimelineDialog`.
- Produces: optional `overlayProps: ComponentProps<typeof Dialog.Overlay>` on the local `Dialog.Content` wrapper; the calendar dialog supplies `{ onclick: () => (open = false) }`.

- [ ] **Step 1: Write the failing mobile browser regression test**

Create `frontend-school/tests/e2e/calendar-embed-dialog.spec.ts`:

```ts
import { devices, expect, test } from '@playwright/test';

test.use({ ...devices['iPhone 13'] });

test('closes the embedded calendar day dialog when its overlay is tapped', async ({ page }) => {
	await page.route('**/api/public/calendar/events?*', async (route) => {
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			body: JSON.stringify({ success: true, data: [] })
		});
	});

	await page.goto('/calendar/embed');
	await page.locator('button[aria-label*="กิจกรรม"]').first().tap();

	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();

	await page.locator('[data-slot="dialog-overlay"]').tap({ position: { x: 8, y: 8 } });

	await expect(dialog).toBeHidden();
});
```

- [ ] **Step 2: Run the focused test and confirm the current touch failure**

Start the local frontend in one terminal:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run dev -- --host 127.0.0.1 --port 4173
```

Run the focused test in another terminal:

```bash
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/calendar-embed-dialog.spec.ts --project=chromium
```

Expected: FAIL at `await expect(dialog).toBeHidden()` because the touch overlay interaction leaves the dialog visible.

- [ ] **Step 3: Forward typed overlay properties from the shared content wrapper**

Update the property destructuring and type extension in `frontend-school/src/lib/components/ui/dialog/dialog-content.svelte`:

```ts
let {
	ref = $bindable(null),
	class: className,
	portalProps,
	overlayProps,
	children,
	showCloseButton = true,
	...restProps
}: WithoutChildrenOrChild<DialogPrimitive.ContentProps> & {
	portalProps?: WithoutChildrenOrChild<ComponentProps<typeof DialogPortal>>;
	overlayProps?: ComponentProps<typeof Dialog.Overlay>;
	children: Snippet;
	showCloseButton?: boolean;
} = $props();
```

Forward the property to the existing overlay:

```svelte
<DialogPortal {...portalProps}>
	<Dialog.Overlay {...overlayProps} />
```

- [ ] **Step 4: Opt the calendar day dialog into direct overlay dismissal**

Update `frontend-school/src/lib/components/calendar/CalendarDayTimelineDialog.svelte`:

```svelte
<Dialog.Content
	overlayProps={{ onclick: () => (open = false) }}
	class="flex h-[min(90dvh,46rem)] max-h-[calc(100dvh-1rem)] max-w-[calc(100%-1rem)] flex-col gap-0 overflow-hidden rounded-2xl p-0 sm:max-w-lg"
>
```

Do not add a handler to the shared overlay by default and do not change any other dialog caller.

- [ ] **Step 5: Check the edited Svelte files with the project Svelte tooling**

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer src/lib/components/ui/dialog/dialog-content.svelte --svelte-version 5
npx @sveltejs/mcp svelte-autofixer src/lib/components/calendar/CalendarDayTimelineDialog.svelte --svelte-version 5
```

Expected: both commands report no remaining issues or apply only required formatting.

- [ ] **Step 6: Run the focused mobile regression test again**

With the local frontend still running:

```bash
cd frontend-school
E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/calendar-embed-dialog.spec.ts --project=chromium
```

Expected: PASS; the dialog becomes hidden after the touch overlay tap.

- [ ] **Step 7: Run the frontend verification matrix**

```bash
cd frontend-school
npm run lint
PUBLIC_BACKEND_URL=http://localhost:3000 PUBLIC_VAPID_KEY=test npm run check
npm run test:static
```

Then run from the repository root:

```bash
git diff --check
git diff --stat
git diff -- frontend-school/src/lib/components/ui/dialog/dialog-content.svelte frontend-school/src/lib/components/calendar/CalendarDayTimelineDialog.svelte frontend-school/tests/e2e/calendar-embed-dialog.spec.ts
git status --short
```

Expected: lint, Svelte check, and all static tests pass; no whitespace errors appear; the diff contains only the planned dialog and regression-test changes.

- [ ] **Step 8: Commit the verified implementation**

```bash
git add frontend-school/src/lib/components/ui/dialog/dialog-content.svelte frontend-school/src/lib/components/calendar/CalendarDayTimelineDialog.svelte frontend-school/tests/e2e/calendar-embed-dialog.spec.ts
git commit -m "fix: dismiss calendar day dialog on touch"
```
