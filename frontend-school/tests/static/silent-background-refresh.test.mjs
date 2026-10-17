import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { parse } from 'svelte/compiler';

const sourceRoot = fileURLToPath(new URL('../../src/', import.meta.url));
async function svelteFiles(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	return (
		await Promise.all(
			entries.map(async (entry) => {
				const file = path.join(directory, entry.name);
				return entry.isDirectory() ? svelteFiles(file) : file.endsWith('.svelte') ? [file] : [];
			})
		)
	).flat();
}
function visibleText(node) {
	if (!node || typeof node !== 'object') return '';
	if (node.type === 'Text') return node.data;
	return Object.entries(node)
		.filter(([key]) => !['attributes', 'expression', 'loc', 'metadata'].includes(key))
		.flatMap(([, value]) => (Array.isArray(value) ? value : [value]))
		.map(visibleText)
		.join(' ');
}
const routineLabel =
	/^(?:รีเฟรช.*|โหลดใหม่|โหลดข้อมูลใหม่|โหลดผลล่าสุด|โหลดล่าสุด|ตรวจข้อมูลล่าสุด|โหลดวิชาสอบใหม่|โหลดคะแนนใหม่|โหลดห้องสอบใหม่|โหลดรอบใหม่|โหลดข้อมูลรอบใหม่|โหลดใบสมัครใหม่|โหลดรายละเอียดใบสมัครใหม่|โหลดแบบใหม่|โหลดรายชื่อใหม่|Force Refresh Auth|Refresh|Reload)$/i;

test('School markup has no routine page or region refresh controls or background updating labels', async () => {
	const violations = [];
	for (const file of await svelteFiles(sourceRoot)) {
		const source = await readFile(file, 'utf8');
		const ast = parse(source, { modern: true });
		function visit(node) {
			if (!node || typeof node !== 'object') return;
			if (node.type === 'Component' && node.name === 'RegionUpdatingState') {
				violations.push(`${path.relative(sourceRoot, file)}: background updating component`);
			}
			if (
				['Component', 'RegularElement'].includes(node.type) &&
				/^(?:Button|LoadingButton|button)$/.test(node.name)
			) {
				const labels = [visibleText(node.fragment).replace(/\s+/g, ' ').trim()];
				for (const attribute of node.attributes ?? []) {
					if (['aria-label', 'title'].includes(attribute.name) && Array.isArray(attribute.value)) {
						labels.push(attribute.value.map(visibleText).join('').trim());
					}
				}
				// This explicit readiness check appears only after a transition conflict.
				const conflictReadinessCheck =
					path.relative(sourceRoot, file) ===
						'lib/components/academic/lifecycle/TermActivationDialog.svelte' &&
					labels[0] === 'ตรวจข้อมูลล่าสุด' &&
					/\{#if needsRefresh\}/.test(source);
				if (!conflictReadinessCheck && labels.some((label) => routineLabel.test(label))) {
					violations.push(
						`${path.relative(sourceRoot, file)}: routine refresh ${labels.join(' / ')}`
					);
				}
			}
			if (
				node.type === 'RegularElement' &&
				node.attributes?.some(
					(attribute) => attribute.name === 'role' && attribute.value?.[0]?.data === 'status'
				) &&
				/กำลังอัปเดต/.test(visibleText(node.fragment))
			) {
				violations.push(`${path.relative(sourceRoot, file)}: visible background updating status`);
			}
			for (const [key, value] of Object.entries(node)) {
				if (['loc', 'metadata', 'expression', 'attributes'].includes(key)) continue;
				if (Array.isArray(value)) value.forEach(visit);
				else if (value && typeof value === 'object') visit(value);
			}
		}
		visit(ast.fragment);
	}
	assert.deepEqual(violations, []);
});

test('silent menu revalidation retains its busy region, initial skeleton, and error-only retry', async () => {
	const sidebar = await readFile(
		path.join(sourceRoot, 'lib/components/layout/Sidebar.svelte'),
		'utf8'
	);
	assert.match(sidebar, /aria-busy=\{menuLoading\}/);
	assert.match(sidebar, /menuLoading && !appMenu.loaded/);
	assert.match(sidebar, /Array\(6\)[^]*?animate-pulse/);
	assert.match(sidebar, /appMenu.error[^]*?onaction=\{appMenu.retry\}/);
	assert.doesNotMatch(sidebar, /กำลังอัปเดตเมนู|RegionUpdatingState/);
});
