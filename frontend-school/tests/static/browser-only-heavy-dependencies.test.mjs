import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

async function readProjectFile(relativePath) {
	return readFile(path.join(projectRoot, relativePath), 'utf8');
}

test('large browser-only libraries resolve to an SSR stub', async () => {
	const viteConfig = await readProjectFile('vite.config.ts');
	const serverStub = await readProjectFile('src/lib/utils/browser-only-heavy-dependency.server.ts');

	assert.match(viteConfig, /client-only-heavy-dependencies/);
	assert.match(viteConfig, /this\.environment\.name === 'ssr'/);
	assert.match(viteConfig, /browserOnlyHeavyDependencyServerStub/);
	for (const dependency of [
		'exceljs',
		'heic2any',
		'pdf-lib',
		'pdfjs-dist',
		'pdfmake/build/pdfmake',
		'qrcode',
		'xlsx'
	]) {
		assert.match(viteConfig, new RegExp(`['"]${dependency.replace('/', '\\/')}['"]`));
		assert.doesNotMatch(serverStub, new RegExp(`from ['"]${dependency.replace('/', '\\/')}['"]`));
	}
	assert.match(serverStub, /ฟังก์ชันส่งออกและแปลงไฟล์ใช้งานได้เฉพาะในเบราว์เซอร์/);
});

test('certificate renderer has one lazy UI boundary and an SSR-only stub', async () => {
	const viteConfig = await readProjectFile('vite.config.ts');
	const publicBoundary = await readProjectFile('src/lib/certificates/renderer.ts');
	const serverStub = await readProjectFile('src/lib/certificates/renderer.server.ts');

	assert.match(viteConfig, /client-only-certificate-renderer/);
	assert.match(viteConfig, /certificateRendererServerStub/);
	assert.match(viteConfig, /this\.environment\.name === 'ssr'/);
	assert.match(publicBoundary, /await import\('\.\/renderer\.browser'\)/);
	for (const dependency of ['pdf-lib', 'pdfjs-dist', 'qrcode']) {
		assert.doesNotMatch(publicBoundary, new RegExp(`from ['"]${dependency}['"]`));
		assert.doesNotMatch(serverStub, new RegExp(`from ['"]${dependency}['"]`));
	}
	assert.match(serverStub, /ตัวสร้างเกียรติบัตรใช้งานได้เฉพาะในเบราว์เซอร์/);
});

test('HEIC conversion is loaded only when a browser user selects a HEIC file', async () => {
	const components = await Promise.all([
		readProjectFile('src/lib/components/achievement/AchievementDialog.svelte'),
		readProjectFile('src/lib/components/forms/ProfileImageUpload.svelte')
	]);

	for (const component of components) {
		assert.match(component, /import \{ browser \} from '\$app\/environment'/);
		assert.match(component, /if \(!browser\) throw new Error/);
		assert.match(component, /await import\('heic2any'\)/);
		assert.doesNotMatch(component, /import heic2any from 'heic2any'/);
	}
});
