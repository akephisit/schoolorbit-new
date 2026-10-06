import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';
const root = path.resolve(import.meta.dirname, '../..');
const read = (name) => readFile(path.join(root, name), 'utf8');
test('delivery links the exact edition, level, program and delivery context', async () => {
	const workspace = await read(
		'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte'
	);
	for (const text of [
		'ตรงกับหลักสูตร',
		'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน',
		'เปิดสอนเพิ่มเติมนอกหลักสูตร',
		'หยุดสอนก่อนรุ่นเปิดสอนนี้มีผล',
		'คาบจริงต่างจากค่ามาตรฐานในหลักสูตร'
	])
		assert.ok(workspace.includes(text));
	for (const field of [
		'room.studyProgram.editionId',
		'room.curriculumLevelId',
		'room.extraOfferings',
		'workspace.deliveryVersionId',
		'academicYearId',
		'academicTermId',
		'studyProgramId'
	])
		assert.ok(workspace.includes(field));
	assert.match(workspace, /\/levels\/\$\{room\.curriculumLevelId\}/);
	assert.doesNotMatch(workspace, /getLearningOffering|getLearningGroup|listLearningGroups/);
});
test('level alignment is an independent read-only region', async () => {
	const loader = await read(
		'src/routes/(app)/staff/academic/curricula/[id]/levels/[levelId]/+page.ts'
	);
	const page = await read(
		'src/routes/(app)/staff/academic/curricula/[id]/levels/[levelId]/+page.svelte'
	);
	const panel = await read(
		'src/lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte'
	);
	assert.match(loader, /getHomeroomDeliveryWorkspace/);
	assert.match(loader, /captureRouteLoad/);
	assert.match(page, /data\.alignment/);
	assert.match(page, /alignmentError/);
	assert.match(page, /retryAlignment/);
	assert.match(panel, /room\.studyProgram\.curriculumLevelId === curriculumLevelId/);
	assert.match(panel, /กลับไปจัดการการเปิดสอน/);
	assert.match(panel, /workspace\.deliveryVersionId/);
	assert.doesNotMatch(
		panel,
		/getLearningOffering|getLearningGroup|listLearningGroups|copyStudyProgram/
	);
});
