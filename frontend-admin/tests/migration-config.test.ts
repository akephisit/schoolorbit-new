import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';
for (const [route, method] of [
	['status', 'GET'],
	['migrate-all', 'POST']
]) {
	test(`retired ${route} proxy refuses access without contacting internal services`, async () => {
		const source = await readFile(
			new URL(`../src/routes/api/migration/${route}/+server.ts`, import.meta.url),
			'utf8'
		);
		const compiled = ts.transpileModule(source, {
			compilerOptions: { module: ts.ModuleKind.ESNext }
		}).outputText;
		const module = await import(
			`data:text/javascript;base64,${Buffer.from(compiled).toString('base64')}`
		);
		const response = await module[method]();
		assert.equal(response.status, 410);
		assert.equal((await response.json()).success, false);
	});
}
