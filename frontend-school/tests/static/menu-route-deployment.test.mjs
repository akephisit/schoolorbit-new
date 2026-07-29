import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const repoRoot = path.resolve(projectRoot, '..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('menu synchronization is an explicit fail-visible post-deployment step', async () => {
	const viteConfig = await readProjectFile('vite.config.ts');
	const packageJson = JSON.parse(await readProjectFile('package.json'));
	const workflows = await Promise.all([
		readRepoFile('.github/workflows/deploy-school-tenant.yml'),
		readRepoFile('.github/workflows/deploy-all-schools.yml')
	]);

	assert.equal(
		packageJson.scripts['sync:menu-routes'],
		'node --experimental-strip-types scripts/register-menu-routes.ts'
	);
	assert.doesNotMatch(viteConfig, /menuRegistryPlugin|VITE_DEPLOY_KEY|scanRoutes/);

	for (const workflow of workflows) {
		assert.doesNotMatch(workflow, /VITE_DEPLOY_KEY/);
		assert.match(workflow, /name: Synchronize menu routes/);
		assert.match(workflow, /DEPLOY_KEY:\s*\$\{\{\s*secrets\.DEPLOY_KEY\s*\}\}/);
		assert.match(workflow, /run: npm run sync:menu-routes/);
		assert.ok(
			workflow.indexOf('cloudflare/wrangler-action') <
				workflow.indexOf('run: npm run sync:menu-routes'),
			'menu synchronization must run after the tenant deployment'
		);
	}
});
