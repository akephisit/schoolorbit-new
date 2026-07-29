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
- Modify: `frontend-school/tests/static/file-platform-contract.test.mjs`
- Modify: `frontend-school/src/lib/components/files/PrivateFileImage.svelte`

**Interfaces:**
- Consumes: `downloadFile(fileId: string, resourceId?: string, signal?: AbortSignal) -> Promise<Blob>`.
- Preserves: `PrivateFileImage` props `fileId`, `resourceId`, `alt`, and `class`.
- Produces: an `<img>` that is hidden initially and becomes visible only after its blob source loads successfully.

- [ ] **Step 1: Add the focused regression test**

Add this test to `frontend-school/tests/static/file-platform-contract.test.mjs`:

```javascript
test('private file images remain hidden until the downloaded blob loads', async () => {
	const source = await readRepoFile(
		'frontend-school/src/lib/components/files/PrivateFileImage.svelte'
	);

	assert.match(source, /<img[^>]*style:visibility=["']hidden["']/s);
	assert.match(source, /node\.addEventListener\(['"]load['"],\s*revealImage\)/);
	assert.match(source, /node\.style\.visibility\s*=\s*['"]visible['"]/);
	assert.match(source, /node\.removeEventListener\(['"]load['"],\s*revealImage\)/);
});
```

- [ ] **Step 2: Run the focused test and confirm RED**

Run:

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
```

Expected: the new test fails because the current image is not initially hidden and has no load listener.

- [ ] **Step 3: Validate the Svelte pattern before editing**

Use the official Svelte documentation tools to confirm attachment cleanup and DOM event handling. Run the official Svelte autofixer against the current component and retain any relevant diagnostics.

- [ ] **Step 4: Implement the minimal load-state behavior**

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

- [ ] **Step 5: Run the official Svelte autofixer again**

Run the official Svelte autofixer against the corrected component. Apply only diagnostics relevant to this change and repeat until it reports no issue.

- [ ] **Step 6: Run the focused test and confirm GREEN**

Run:

```bash
cd frontend-school
node --test tests/static/file-platform-contract.test.mjs
```

Expected: all file-platform contract tests pass.

- [ ] **Step 7: Commit the implementation**

```bash
git add frontend-school/tests/static/file-platform-contract.test.mjs \
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

- [ ] **Step 1: Run frontend verification**

Run:

```bash
cd frontend-school
PUBLIC_BACKEND_URL=https://school-api.schoolorbit.app npm run check
npm run lint
npm run test:static
```

Expected: Svelte Check reports 0 errors and 0 warnings; lint and all static tests pass.

- [ ] **Step 2: Review the completed diff**

Run:

```bash
git diff HEAD~1 --check
git diff HEAD~1 --stat
git status --short
```

Expected: no whitespace errors; only the test and shared component are changed after the design/plan documentation commits; the worktree is clean.

- [ ] **Step 3: Push the verified commits**

Run:

```bash
git push origin main
```

Expected: `origin/main` advances to the local verified commit.

- [ ] **Step 4: Deploy only the affected tenant**

Run:

```bash
gh workflow run deploy-school-tenant.yml \
  -f subdomain=snwsb \
  -f school_id=0e297ca4-0809-4aab-a03f-1915045257b8 \
  -f api_url=https://school-api.schoolorbit.app
```

Watch the dispatched run with `gh run watch <run-id> --exit-status`.

Expected: the workflow finishes successfully and deploys `snwsb`.

- [ ] **Step 5: Verify the production loading transition**

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

- [ ] **Step 6: Confirm final repository state**

Run:

```bash
git status --short
git rev-parse HEAD
git rev-parse origin/main
```

Expected: the worktree is clean and local `HEAD` equals `origin/main`.
