import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('authenticated app routes opt into CSR', async () => {
	const source = await readProjectFile('src/routes/(app)/+layout.ts');

	assert.match(source, /export\s+const\s+ssr\s*=\s*false\s*;?/);
});

test('protected app layout redirects unauthenticated users before rendering children', async () => {
	const source = await readProjectFile('src/routes/(app)/+layout.svelte');

	assert.match(source, /from\s+['"]\$app\/navigation['"]/);
	assert.match(source, /\bgoto\(/);
	assert.match(source, /redirectAfterLogin/);
	assert.match(source, /authStatus\s*===\s*['"]authenticated['"]/);
});

test('protected app layout sends logged-out sessions to login instead of forbidden', async () => {
	const source = await readProjectFile('src/routes/(app)/+layout.svelte');

	assert.match(source, /async function redirectToLogin/);
	assert.match(source, /if\s*\(!user\)\s*\{[\s\S]*redirectToLogin\(\)[\s\S]*return;[\s\S]*\}/);
	assert.match(source, /if\s*\(userCanAccessRoute\(user,\s*permissions,\s*routeId\)\)\s*return;/);
	assert.match(source, /void redirectToForbidden\(\)/);
	assert.match(
		source,
		/if\s*\(!user\)\s*\{[\s\S]*redirectToLogin\(\)[\s\S]*return;[\s\S]*if\s*\(userCanAccessRoute/
	);
});

test('svelte DOM bind:this refs use stateful variables when reassigned', async () => {
	const studentIdsPage = await readProjectFile(
		'src/routes/(app)/staff/academic/admission/[id]/student-ids/+page.svelte'
	);
	const timetablePage = await readProjectFile(
		'src/routes/(app)/staff/academic/timetable/+page.svelte'
	);

	assert.match(studentIdsPage, /let\s+fileInput\s*=\s*\$state<HTMLInputElement>\(\)/);
	assert.doesNotMatch(studentIdsPage, /let\s+fileInput\s*:\s*HTMLInputElement/);
	assert.match(timetablePage, /let\s+workspaceRef\s*=\s*\$state<HTMLDivElement>\(\)/);
	assert.doesNotMatch(timetablePage, /let\s+workspaceRef\s*:\s*HTMLDivElement/);
});

test('api client parses non-json responses and centralizes unauthorized handling', async () => {
	const source = await readProjectFile('src/lib/api/client.ts');

	assert.match(source, /headers\.get\(['"]content-type['"]\)/);
	assert.match(source, /response\.status\s*===\s*401/);
	assert.match(source, /sessionStorage\.setItem\(['"]redirectAfterLogin['"]/);
});
