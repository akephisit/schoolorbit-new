# Private File Image Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent private profile images from showing a broken-image icon while their blob URL is loading.

**Architecture:** Keep `PrivateFileImage` as the shared delivery component and preserve its existing `<img>` contract. Hide the image from the initial render, reveal it only after the browser emits `load`, and leave the consumer's existing background visible during loading or failure.

**Tech Stack:** Svelte 5 attachments, TypeScript, Node test runner, Svelte Check, ESLint, Prettier, Playwright, GitHub Actions, Cloudflare Workers.

## Global Constraints

- Change only the shared private-image presentation behavior and its regression test.
- Keep the private file API, authorization, storage provider, and download flow unchanged.
- Preserve existing image classes and surrounding layouts.
- Do not modify backend-admin or frontend-admin.
- Never log signed URLs, credentials, national IDs, object keys, bucket names, or raw request bodies.
- Execute inline in the current worktree, as requested by the user.

---

### Task 1: Hide Private Images Until Browser Load

**Files:**
- Create: `frontend-school/tests/e2e/private-file-image.spec.ts`
- Modify: `frontend-school/src/lib/components/files/PrivateFileImage.svelte`

**Interfaces:**
- Consumes: `downloadFile(fileId: string, resourceId?: string, signal?: AbortSignal) -> Promise<Blob>`.
- Preserves: `PrivateFileImage` props `fileId`, `resourceId`, `alt`, and `class`.
- Produces: an `<img>` that is hidden initially and becomes visible only after its blob source loads successfully.

- [x] **Step 1: Add a browser-level regression test**

Create `frontend-school/tests/e2e/private-file-image.spec.ts`. The test starts a
local Vite server with a virtual page that mounts the real component, holds the
real component's grant request, and controls a valid one-pixel image response:

```typescript
import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, type Plugin, type ViteDevServer } from 'vite';

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const harnessPath = '/__private-file-image-test';
const virtualModuleId = 'virtual:private-file-image-test';
const resolvedVirtualModuleId = `\0${virtualModuleId}`;
const stubModulePrefix = '\0private-file-image-test-stub:';
const stubModules = new Map([
	[
		'$app/environment',
		'export const browser = true; export const building = false; export const dev = true;'
	],
	[
		'$app/paths',
		"export const base = ''; export const assets = ''; export const resolve = (path) => path;"
	],
	['$env/dynamic/public', 'export const env = {};'],
	['$env/static/public', "export const PUBLIC_BACKEND_URL = 'https://school-api.schoolorbit.app';"]
]);
const png = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=',
	'base64'
);

function harnessPlugin(): Plugin {
	return {
		name: 'private-file-image-test-harness',
		enforce: 'pre',
		resolveId(id) {
			if (id === virtualModuleId) return resolvedVirtualModuleId;
			if (stubModules.has(id)) return `${stubModulePrefix}${id}`;
		},
		load(id) {
			if (id.startsWith(stubModulePrefix)) {
				return stubModules.get(id.slice(stubModulePrefix.length));
			}
			if (id !== resolvedVirtualModuleId) return;
			return `
				import { mount } from 'svelte';
				import PrivateFileImage from '/src/lib/components/files/PrivateFileImage.svelte';
				mount(PrivateFileImage, {
					target: document.querySelector('#app'),
					props: { fileId: 'test-file', alt: 'Profile' }
				});
			`;
		},
		configureServer(server) {
			server.middlewares.use((request, response, next) => {
				const pathname = new URL(request.url ?? '/', 'http://test').pathname;
				if (pathname !== harnessPath) return next();
				response.setHeader('Content-Type', 'text/html; charset=utf-8');
				response.end(
					`<div id="app"></div><script type="module" src="/@id/${virtualModuleId}"></script>`
				);
			});
		}
	};
}

let devServer: ViteDevServer;
let baseUrl: string;

test.beforeAll(async () => {
	devServer = await createServer({
		root: frontendRoot,
		logLevel: 'silent',
		plugins: [harnessPlugin()],
		server: { host: '127.0.0.1', port: 0 }
	});
	await devServer.listen();
	const address = devServer.httpServer?.address();
	if (!address || typeof address === 'string') throw new Error('Vite test server did not start');
	baseUrl = `http://127.0.0.1:${address.port}`;
});

test.afterAll(async () => {
	await devServer.close();
});

test('keeps a private image hidden until the downloaded blob loads', async ({ page }) => {
	let releaseGrant = () => {};
	const heldGrant = new Promise<void>((resolve) => {
		releaseGrant = resolve;
	});
	let markGrantRequested = () => {};
	const grantRequested = new Promise<void>((resolve) => {
		markGrantRequested = resolve;
	});

	await page.route('https://school-api.schoolorbit.app/api/files/test-file/download', async (route) => {
		markGrantRequested();
		await heldGrant;
		await route.fulfill({
			status: 200,
			contentType: 'application/json',
			headers: {
				'Access-Control-Allow-Origin': baseUrl,
				'Access-Control-Allow-Credentials': 'true'
			},
			body: JSON.stringify({
				success: true,
				data: {
					url: `${baseUrl}/__private-file-image.png`,
					expiresAt: '2099-01-01T00:00:00Z'
				}
			})
		});
	});
	await page.route(`${baseUrl}/__private-file-image.png`, (route) =>
		route.fulfill({ status: 200, contentType: 'image/png', body: png })
	);

	await page.goto(`${baseUrl}${harnessPath}`);
	await grantRequested;
	const image = page.locator('img[alt="Profile"]');

	try {
		await expect(image).toHaveCSS('visibility', 'hidden');
	} finally {
		releaseGrant();
	}

	await expect(image).toHaveCSS('visibility', 'visible');
	await expect
		.poll(() => image.evaluate((node) => (node as HTMLImageElement).naturalWidth))
		.toBe(1);
});
```

- [x] **Step 2: Run the focused test and confirm RED**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/private-file-image.spec.ts
```

Expected: the browser test fails at `visibility: hidden` because the current image
is visible while its grant request is held.

- [x] **Step 3: Validate the Svelte pattern before editing**

Use the official Svelte documentation tools to confirm attachment cleanup and DOM event handling. Run the official Svelte autofixer against the current component and retain any relevant diagnostics.

- [x] **Step 4: Implement the minimal load-state behavior**

In `PrivateFileImage.svelte`, initialize the node as hidden, register the image load listener before assigning the object URL, and fully clean up:

```typescript
node.style.visibility = 'hidden';

function revealImage() {
	if (!controller.signal.aborted) {
		node.style.visibility = 'visible';
	}
}

node.addEventListener('load', revealImage);

function cleanup() {
	controller.abort();
	node.removeEventListener('load', revealImage);
	if (objectUrl) {
		URL.revokeObjectURL(objectUrl);
		objectUrl = null;
	}
	node.removeAttribute('src');
	node.style.visibility = 'hidden';
}
```

Render the initial hidden state in the markup so it is present before the attachment runs:

```svelte
<img
	style:visibility="hidden"
	{@attach privateFileImage({ fileId, resourceId })}
	{alt}
	class={className}
/>
```

Keep the current download error logging. A failed download leaves the image hidden and exposes the existing consumer background.

- [x] **Step 5: Run the official Svelte autofixer again**

Run the official Svelte autofixer against the corrected component. Apply only diagnostics relevant to this change and repeat until it reports no issue.

- [x] **Step 6: Run the focused test and confirm GREEN**

Run:

```bash
cd frontend-school
npx playwright test tests/e2e/private-file-image.spec.ts
```

Expected: the component is hidden while the grant is held, becomes visible after
the valid PNG loads, and the focused browser test passes.

- [x] **Step 7: Commit the implementation**

```bash
git add frontend-school/tests/e2e/private-file-image.spec.ts \
  frontend-school/src/lib/components/files/PrivateFileImage.svelte
git commit -m "fix: hide private images until loaded"
```

---

### Task 2: Verify and Deploy the School Frontend

**Files:**
- Verify: `frontend-school`
- Deploy: `.github/workflows/deploy-school-tenant.yml`

**Interfaces:**
- Consumes: the Task 1 `PrivateFileImage` behavior.
- Produces: the verified frontend deployment at `https://snwsb.schoolorbit.app`.

- [x] **Step 1: Run frontend verification**

Run:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=https://school-api.schoolorbit.app npm run check
npm run lint
npm run test:static
```

Expected: Svelte Check reports 0 errors and 0 warnings; lint and all static tests pass.

- [x] **Step 2: Review the completed diff**

Run:

```bash
git diff HEAD~1 --check
git diff HEAD~1 --stat
git status --short
```

Expected: no whitespace errors; only the test and shared component are changed after the design/plan documentation commits; the worktree is clean.

- [x] **Step 3: Push the verified commits**

Run:

```bash
git push origin main
```

Expected: `origin/main` advances to the local verified commit.

- [x] **Step 4: Deploy only the affected tenant**

Run:

```bash
gh workflow run deploy-school-tenant.yml \
  -f subdomain=snwsb \
  -f school_id=0e297ca4-0809-4aab-a03f-1915045257b8 \
  -f api_url=https://school-api.schoolorbit.app
```

Watch the dispatched run with `gh run watch <run-id> --exit-status`.

Expected: the workflow finishes successfully and deploys `snwsb`.

- [x] **Step 5: Verify the production loading transition**

Use Playwright with an authenticated `snwsb` session. Hold requests matching
`https://school-api.schoolorbit.app/api/files/*/download*`, refresh the page, and assert:

```javascript
const profileImage = page.locator('img[alt="Profile"]').first();
await expect(profileImage).toHaveCSS('visibility', 'hidden');
```

Release the held request and assert:

```javascript
await expect(profileImage).toHaveCSS('visibility', 'visible');
await expect
	.poll(() => profileImage.evaluate((image) => (image as HTMLImageElement).naturalWidth))
	.toBeGreaterThan(0);
```

Also confirm the private download grant returns `200` and the R2 image fetch has no CORS failure. Do not print or persist its signed URL.

- [x] **Step 6: Confirm final repository state**

Run:

```bash
git status --short
git rev-parse HEAD
git rev-parse origin/main
```

Expected: the worktree is clean and local `HEAD` equals `origin/main`.
