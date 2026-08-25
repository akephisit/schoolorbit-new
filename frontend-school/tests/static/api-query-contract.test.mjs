import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function importTypescript(relativePath) {
	const source = await readFile(path.join(projectRoot, relativePath), 'utf8');
	const output = ts.transpileModule(source, {
		compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
		fileName: relativePath
	}).outputText;
	return import(
		`data:text/javascript;base64,${Buffer.from(output).toString('base64')}#${Date.now()}`
	);
}

test('appendApiQuery encodes defined scalars and existing queries', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.equal(
		appendApiQuery('/api/students', {
			academicYearId: 'year/1',
			page: 2,
			activeOnly: false
		}),
		'/api/students?academicYearId=year%2F1&page=2&activeOnly=false'
	);
	assert.equal(
		appendApiQuery('/api/students?view=compact', { pageSize: 20 }),
		'/api/students?view=compact&pageSize=20'
	);
});

test('appendApiQuery repeats arrays and omits absent values', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.equal(
		appendApiQuery('/api/students', {
			search: undefined,
			status: null,
			tagId: ['tag-a', 'tag-b']
		}),
		'/api/students?tagId=tag-a&tagId=tag-b'
	);
});

test('appendApiQuery rejects non-scalar query values', async () => {
	const { appendApiQuery } = await importTypescript('src/lib/api/query.ts');
	assert.throws(() => appendApiQuery('/api/students', { filter: { status: 'active' } }));
});
