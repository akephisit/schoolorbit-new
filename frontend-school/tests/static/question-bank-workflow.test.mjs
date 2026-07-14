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
		page.indexOf('function cleanupDraftObjectUrls')
	);
	const save = page.slice(
		page.indexOf('async function saveQuestion'),
		page.indexOf('function requestDelete')
	);

	assert.match(selection, /URL\.createObjectURL\(file\)/);
	assert.doesNotMatch(selection, /uploadFile\(/);
	assert.match(save, /uploadFile\(image\.file, 'course_material', true\)/);
	assert.match(save, /uploadedFileIds\.set\(image\.pendingId, response\.file\.id\)/);
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
	const extensions = await readProjectFile('src/lib/question-bank/rich-editor-extensions.ts');

	assert.match(packageJson, /"mathlive":/);
	assert.match(packageJson, /"@tiptap\/core":/);
	assert.match(page, /<QuestionContentEditor/);
	assert.doesNotMatch(page, /สมการ LaTeX|placeholder="LaTeX/);
	assert.match(editor, />\s*ข้อความ\s*</);
	assert.match(editor, />\s*สมการ\s*</);
	assert.match(editor, />\s*รูปภาพ\s*</);
	assert.match(extensions, /document\.createElement\('math-field'\)/);
	assert.match(editor, /field\.insert/);
	assert.match(editor, /window\.mathVirtualKeyboard\.show/);
	assert.match(editor, /window\.mathVirtualKeyboard\.hide/);
	assert.match(editor, /keyboardVisible \? 'secondary' : 'outline'/);
	assert.match(page, /onInteractOutside=\{handleDialogInteractOutside\}/);
	assert.match(page, /target\.closest\('\.ML__keyboard'\)/);
	assert.doesNotMatch(page, /event\s*\.composedPath\(\)/);
	assert.match(page, /if \(fromMathKeyboard\) event\.preventDefault\(\)/);
	assert.match(page, /keyboard\.container = node/);
	assert.match(page, /\{@attach connectMathVirtualKeyboardContainer\}/);
	assert.match(page, /keyboard\.container = document\.body/);
	assert.doesNotMatch(page, /trapFocus=\{false\}/);
	assert.match(extensions, /mathfieldConstructor\.soundsDirectory = null/);
	assert.match(editor, /import \{ untrack \} from 'svelte'/);
	assert.match(editor, /function connectEditor[\s\S]*return untrack\(\(\) => \{/);
	assert.match(editor, /เศษส่วน/);
	assert.match(editor, /รากที่สอง/);
});

test('question content uses a versioned JSON document and strips editor-only image data', async () => {
	const api = await readProjectFile('src/lib/api/questionBank.ts');
	const documentHelpers = await readProjectFile('src/lib/question-bank/rich-document.ts');
	const extensions = await readProjectFile('src/lib/question-bank/rich-editor-extensions.ts');
	const renderer = await readProjectFile('src/lib/components/question-bank/QuestionContent.svelte');

	assert.match(api, /schemaVersion:\s*1/);
	assert.match(api, /type:\s*'inline_math'/);
	assert.match(api, /type:\s*'image'/);
	assert.match(documentHelpers, /toPersistedRichContent/);
	assert.match(documentHelpers, /const fileId = block\.attrs\.fileId \?\?/);
	assert.match(documentHelpers, /attrs:\s*\{\s*fileId,/);
	assert.match(extensions, /draggable:\s*true/);
	assert.match(extensions, /insertContentAt\(position, nodes\)/);
	assert.doesNotMatch(renderer, /\{@html/);
});

test('question search uses the plain-text projection added by a new migration', async () => {
	const migration = await readProjectFile(
		'../backend-school/migrations/025_question_bank_rich_document.sql'
	);
	const services = await readProjectFile('../backend-school/src/modules/question_bank/services.rs');

	assert.match(migration, /ADD COLUMN search_text TEXT NOT NULL/);
	assert.match(migration, /idx_question_bank_questions_search_trgm/);
	assert.match(migration, /schemaVersion/);
	assert.match(services, /let stem_search_text = payload\.stem_content\.search_text\(\)/);
	assert.match(services, /q\.search_text ILIKE/);
	assert.doesNotMatch(services, /q\.stem_content::text ILIKE/);
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
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');

	assert.match(math, /katex\.render/);
	assert.match(math, /trust:\s*false/);
	assert.match(math, /throwOnError:\s*false/);
	assert.match(math, /question-math--inline/);
	assert.match(math, /font-size:\s*1em/);
	assert.match(math, /vertical-align:\s*baseline/);
	assert.match(content, /<MathContent/);
	assert.doesNotMatch(content, /align-middle/);
	assert.match(page, /<QuestionContent content=\{question\.stemContent\} compact \/>/);
	assert.doesNotMatch(page, /questionTitle\(/);
});

test('question bank exports selected questions to Word with embedded image formulas', async () => {
	const packageJson = await readProjectFile('package.json');
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');
	const api = await readProjectFile('src/lib/api/questionBank.ts');
	const apiClient = await readProjectFile('src/lib/api/client.ts');
	const exporter = await readProjectFile('src/lib/question-bank/word-export.ts');
	const formulaFonts = await readProjectFile('src/lib/question-bank/katex-fonts.ts');
	const backendRoutes = await readProjectFile('../backend-school/src/modules/question_bank.rs');
	const backendHandlers = await readProjectFile(
		'../backend-school/src/modules/question_bank/handlers.rs'
	);
	const backendServices = await readProjectFile(
		'../backend-school/src/modules/question_bank/services.rs'
	);
	const r2Client = await readProjectFile('../backend-school/src/services/r2_client.rs');

	assert.match(packageJson, /"docx":/);
	assert.match(packageJson, /"html-to-image":/);
	assert.match(page, /new SvelteSet<string>\(\)/);
	assert.match(page, /เลือกทั้งหมดในหน้านี้/);
	assert.match(page, /ส่งออก Word/);
	assert.match(page, /loadSelectedQuestionDetails/);
	assert.match(page, /getQuestionBankQuestion\(questionIds\[index\]\)/);
	assert.match(page, /import\('\$lib\/question-bank\/word-export'\)/);
	assert.match(exporter, /Packer\.toBlob\(document\)/);
	assert.match(exporter, /new ImageRun\(/);
	assert.match(exporter, /katex\.render\(/);
	assert.match(exporter, /toPng\(host/);
	assert.match(exporter, /pixelRatio:\s*3/);
	assert.match(exporter, /fontEmbedCSS:\s*katexFontEmbedCss/);
	assert.doesNotMatch(exporter, /getFontEmbedCSS/);
	assert.match(formulaFonts, /\.woff2\?inline/);
	assert.match(exporter, /getQuestionBankQuestionFile\(questionId, fileId\)/);
	assert.doesNotMatch(exporter, /\bfetch\s*\(/);
	assert.match(api, /export async function getQuestionBankQuestionFile/);
	assert.match(api, /apiClient\.getBlob/);
	assert.match(apiClient, /async getBlob\(endpoint: string\)/);
	assert.match(exporter, /includeAnswerKey/);
	assert.match(exporter, /TH Sarabun New/);
	assert.doesNotMatch(exporter, /MathRun|MathFraction|MathEquation/);
	assert.match(backendRoutes, /questions\/\{question_id\}\/files\/\{file_id\}/);
	assert.match(backendHandlers, /get_question_file_source/);
	assert.match(backendHandlers, /private, max-age=300/);
	assert.match(backendServices, /referenced_file_ids\.contains\(&file_id\)/);
	assert.match(backendServices, /mime_type LIKE 'image\/%'/);
	assert.match(r2Client, /pub async fn download_file/);
});
