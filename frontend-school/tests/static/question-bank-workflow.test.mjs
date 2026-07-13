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

test('question bank uses its own subject options and exact subject contract', async () => {
	const api = await readProjectFile('src/lib/api/questionBank.ts');
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');

	assert.match(api, /export async function getQuestionBankOptions/);
	assert.match(api, /\/api\/academic\/question-bank\/options/);
	assert.match(api, /subjectId:\s*string;/);
	assert.doesNotMatch(api, /gradeLevelId/);
	assert.match(page, /getQuestionBankOptions\(\)/);
	assert.doesNotMatch(page, /getAcademicStructure|listSubjects|gradeLevelId|ปีการศึกษา/);
	assert.match(page, /ข้อสอบจะผูกกับรายวิชาที่เลือกโดยตรง/);
});

test('question bank defers image uploads until save and cleans failed temporary files', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');
	const editor = await readProjectFile(
		'src/lib/components/question-bank/QuestionContentEditor.svelte'
	);
	const selection = page.slice(
		page.indexOf('function selectDraftImage'),
		page.indexOf('function removeDraftImage')
	);
	const save = page.slice(
		page.indexOf('async function saveQuestion'),
		page.indexOf('function requestDelete')
	);

	assert.match(selection, /URL\.createObjectURL\(file\)/);
	assert.doesNotMatch(selection, /uploadFile\(/);
	assert.match(save, /uploadFile\(content\.imageFile, 'course_material', true\)/);
	assert.match(save, /if \(!saveRequestStarted\)/);
	assert.match(save, /Promise\.allSettled\(uploadedIds\.map\(\(id\) => deleteFile\(id\)\)\)/);
	assert.match(save, /เก็บกวาดอัตโนมัติภายใน 24 ชั่วโมง/);
	assert.match(editor, /รูปจะอัปโหลดเมื่อกดบันทึกเท่านั้น/);
});

test('question editor offers visual math controls without exposing a LaTeX input', async () => {
	const packageJson = await readProjectFile('package.json');
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');
	const editor = await readProjectFile(
		'src/lib/components/question-bank/QuestionContentEditor.svelte'
	);
	const visualMath = await readProjectFile(
		'src/lib/components/question-bank/VisualMathEditor.svelte'
	);

	assert.match(packageJson, /"mathlive":/);
	assert.match(page, /<QuestionContentEditor/);
	assert.doesNotMatch(page, /สมการ LaTeX|placeholder="LaTeX/);
	assert.match(editor, />\s*ข้อความ\s*</);
	assert.match(editor, />\s*สมการ\s*</);
	assert.match(editor, />\s*รูปภาพ\s*</);
	assert.match(visualMath, /<math-field/);
	assert.match(visualMath, /mathfield\.insert/);
	assert.match(visualMath, /window\.mathVirtualKeyboard\.show/);
	assert.match(visualMath, /window\.mathVirtualKeyboard\.hide/);
	assert.match(visualMath, /keyboardVisible \? 'secondary' : 'outline'/);
	assert.match(page, /onInteractOutside=\{handleDialogInteractOutside\}/);
	assert.match(page, /target\.classList\.contains\('ML__keyboard'\)/);
	assert.match(page, /if \(fromMathKeyboard\) event\.preventDefault\(\)/);
	assert.match(visualMath, /เศษส่วน/);
	assert.match(visualMath, /รากที่สอง/);
});

test('question bank keeps read-only actions separate and confirms deletion', async () => {
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');

	assert.match(page, /openQuestion\(question, 'view'\)/);
	assert.match(page, /\{#if question\.canManage\}/);
	assert.match(page, /<AlertDialog\.Root bind:open=\{deleteDialogOpen\}>/);
	assert.match(page, /ยืนยันการลบข้อสอบ/);
	assert.match(page, /pageSize/);
	assert.match(page, /loadQuestions\(currentPage \+ 1\)/);
});

test('question bank renders formulas through KaTeX with untrusted commands disabled', async () => {
	const math = await readProjectFile('src/lib/components/question-bank/MathContent.svelte');
	const content = await readProjectFile('src/lib/components/question-bank/QuestionContent.svelte');

	assert.match(math, /katex\.render/);
	assert.match(math, /trust:\s*false/);
	assert.match(math, /throwOnError:\s*false/);
	assert.match(content, /<MathContent/);
});
