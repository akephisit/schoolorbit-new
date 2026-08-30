import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const projectRoot = path.resolve(import.meta.dirname, '../..');
const readProjectFile = (relativePath) => readFile(path.join(projectRoot, relativePath), 'utf8');

test('delivery renders set-based curriculum alignment and exact context links', async () => {
	const page = await readProjectFile(
		'src/lib/components/learning-delivery/HomeroomDeliveryWorkspace.svelte'
	);

	for (const copy of [
		'ตรงกับหลักสูตร',
		'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน',
		'เปิดสอนเพิ่มเติมนอกหลักสูตร',
		'หยุดสอนก่อนรุ่นตารางนี้มีผล',
		'คาบจริงต่างจากค่ามาตรฐานในหลักสูตร'
	]) {
		assert.match(page, new RegExp(copy));
	}
	assert.match(page, /room\.extraOfferings/);
	assert.match(page, /room\.curriculumVersionId/);
	assert.match(page, /workspace\.timetableVersionId/);
	assert.match(page, /academicYearId/);
	assert.match(page, /academicTermId/);
	assert.match(page, /studyProgramId/);
	assert.match(page, /versionId/);
	assert.doesNotMatch(page, /getLearningOffering|getLearningGroup|listLearningGroups/);
});

test('curriculum alignment context remains read-only and cloning is an explicit handoff', async () => {
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/curricula/[id]/+page.svelte'
	);
	const panel = await readProjectFile(
		'src/lib/components/academic-core/CurriculumDeliveryAlignmentPanel.svelte'
	);
	const versions = await readProjectFile(
		'src/lib/components/academic-core/CurriculumVersionPanel.svelte'
	);

	assert.match(page, /getHomeroomDeliveryWorkspace/);
	assert.match(page, /timetableVersionId/);
	assert.match(page, /studyProgramId/);
	assert.match(page, /cloneCurriculumVersionDraft/);
	assert.match(panel, /กลับไปจัดการการเปิดสอน/);
	assert.match(panel, /workspace\.timetableVersionId/);
	assert.doesNotMatch(panel, /getLearningOffering|getLearningGroup|listLearningGroups/);
	assert.match(versions, /selectedVersion\?\.version\.status === 'published'/);
	assert.match(versions, /sourceRowVersion/);
	assert.match(versions, /ต้นฉบับที่เผยแพร่จะไม่เปลี่ยน/);
	assert.match(versions, /เริ่มใช้ในปีการศึกษา/);
	assert.doesNotMatch(versions, /onCloneVersion\([\s\S]*as unknown/);
});
