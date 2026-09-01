import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const read = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('whole-school view is a read-only projection of one canonical workspace', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /activeView === 'wholeSchool'/);
	assert.match(page, /overviewDay/);
	assert.match(page, /controller\.workspace\.blocks\.filter/);
	assert.match(page, /blockBelongsToRow\(block, 'homeroom', homeroom\.id\)/);
	assert.match(page, /ภาพรวมทั้งโรงเรียน/);
	assert.match(page, /มุมมองนี้ใช้ตรวจภาพรวม/);
});

test('whole-school matrix keeps sticky workbook headers and opens block details read-only', async () => {
	const page = await read('src/routes/(app)/staff/academic/timetable/+page.svelte');

	assert.match(page, /sticky left-0/);
	assert.match(page, /overflow-x-auto/);
	assert.match(page, /onclick=\{\(\) => openEditor\(block\)\}/);
	assert.match(page, /selectedBlock\?\.blockKind/);
	assert.match(page, /\{#if canEdit\}[\s\S]*บันทึก/);
});
