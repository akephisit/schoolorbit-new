import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { convertLatexToMathMl } from 'mathlive/ssr';
import { mml2omml } from 'mathml2omml';

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

test('question bank exports selected questions with editable native Word Math equations', async () => {
	const packageJson = await readProjectFile('package.json');
	const page = await readProjectFile('src/routes/(app)/staff/academic/question-bank/+page.svelte');
	const api = await readProjectFile('src/lib/api/questionBank.ts');
	const apiClient = await readProjectFile('src/lib/api/client.ts');
	const exporter = await readProjectFile('src/lib/question-bank/word-export.ts');
	const serverExporter = await readProjectFile('src/lib/question-bank/word-export.server.ts');
	const viteConfig = await readProjectFile('vite.config.ts');
	const backendRoutes = await readProjectFile('../backend-school/src/modules/question_bank.rs');
	const backendHandlers = await readProjectFile(
		'../backend-school/src/modules/question_bank/handlers.rs'
	);
	const backendServices = await readProjectFile(
		'../backend-school/src/modules/question_bank/services.rs'
	);
	const r2Client = await readProjectFile('../backend-school/src/services/r2_client.rs');

	assert.match(packageJson, /"docx":/);
	assert.match(packageJson, /"mathml2omml":/);
	assert.doesNotMatch(packageJson, /"html-to-image":/);
	assert.match(page, /new SvelteSet<string>\(\)/);
	assert.match(page, /เลือกทั้งหมดในหน้านี้/);
	assert.match(page, /ส่งออก Word/);
	assert.match(page, /loadSelectedQuestionDetails/);
	assert.match(page, /getQuestionBankQuestion\(questionIds\[index\]\)/);
	assert.match(
		page,
		/const selectedQuestionSummaries = new SvelteMap<string, QuestionSummary>\(\)/
	);
	assert.match(page, /let exportQuestionIds = \$state\.raw<string\[\]>\(\[\]\)/);
	assert.match(page, /exportQuestionIds = \[\.\.\.selectedQuestionIds\]/);
	assert.match(page, /const questionIds = \[\.\.\.exportQuestionIds\]/);
	assert.match(page, /reorderExportQuestion/);
	assert.match(page, /moveExportQuestion/);
	assert.match(page, /draggable=\{!exportingWord\}/);
	assert.match(page, /ลากรายการหรือใช้ปุ่มขึ้นลงเพื่อจัดลำดับก่อนส่งออก/);
	assert.match(page, /aria-label=\{`เลื่อนข้อ \$\{index \+ 1\} ขึ้น`\}/);
	assert.match(page, /aria-label=\{`เลื่อนข้อ \$\{index \+ 1\} ลง`\}/);
	assert.match(page, /import\('\$lib\/question-bank\/word-export'\)/);
	assert.match(page, /const loadWordExporter = browser/);
	assert.match(viteConfig, /client-only-word-exporter/);
	assert.match(viteConfig, /this\.environment\.name === 'ssr'/);
	assert.match(viteConfig, /word-export\.server\.ts/);
	assert.doesNotMatch(
		serverExporter,
		/from ['"](?:docx|html-to-image|katex|mathlive\/ssr|mathml2omml)['"]/
	);
	assert.match(exporter, /Packer\.toBlob\(document\)/);
	assert.match(exporter, /new ImageRun\(/);
	assert.match(exporter, /convertLatexToMathMl\(latex\)/);
	assert.match(exporter, /mml2omml\(mathMl\)/);
	assert.match(exporter, /normalizeWordMathFunctions\(xmlDocument\)/);
	assert.match(exporter, /wordMathFunctionNames/);
	assert.match(exporter, /createElementNS\(mathMlNamespace, 'mi'\)/);
	assert.match(exporter, /setAttribute\('fontstyle', 'normal'\)/);
	assert.match(exporter, /const defaultLineSpacing = 240/);
	assert.match(exporter, /lineRule: LineRuleType\.AUTO/);
	assert.match(exporter, /const applyFunction = '\\u2061'/);
	assert.match(exporter, /normalizeMathMlWeight\(xmlDocument\)/);
	assert.match(exporter, /normalizeWordMathWeight\(xmlDocument\)/);
	assert.doesNotMatch(exporter, /spacingAfter/);
	const paragraphAfterValues = [...exporter.matchAll(/\bafter:\s*(\d+)/g)].map((match) =>
		Number(match[1])
	);
	assert.ok(paragraphAfterValues.length > 0);
	assert.ok(paragraphAfterValues.every((value) => value === 0));
	assert.match(exporter, /children\.unshift\(wordText\(options\.prefix \?\? ''\)\)/);
	assert.doesNotMatch(exporter, /wordText\(options\.prefix \?\? '', \{ bold: true \}\)/);
	assert.doesNotMatch(exporter, /wordText\(`ข้อ \$\{index \+ 1\}`, \{ bold: true \}\)/);
	assert.match(exporter, /parseXml\(omml, 'ไม่สามารถแปลงสมการเป็นรูปแบบ Word Math ได้'\)/);
	assert.match(exporter, /new ImportedXmlComponent\(element\.tagName, attributes\)/);
	const formulaExporter = exporter.slice(
		exporter.indexOf('function wordFormula'),
		exporter.indexOf('function questionImage')
	);
	assert.doesNotMatch(formulaExporter, /ImageRun|toPng|katex\.render/);
	assert.match(exporter, /getQuestionBankQuestionFile\(questionId, fileId\)/);
	assert.doesNotMatch(exporter, /\bfetch\s*\(/);
	assert.match(api, /export async function getQuestionBankQuestionFile/);
	assert.match(api, /apiClient\.getBlob/);
	assert.match(apiClient, /async getBlob\(endpoint: string\)/);
	assert.match(exporter, /includeAnswerKey/);
	assert.match(exporter, /TH Sarabun New/);
	assert.doesNotMatch(page, /สูตรจะเป็นภาพ PNG/);
	assert.match(backendRoutes, /questions\/\{question_id\}\/files\/\{file_id\}/);
	assert.match(backendHandlers, /get_question_file_source/);
	assert.match(backendHandlers, /private, max-age=300/);
	assert.match(backendServices, /referenced_file_ids\.contains\(&file_id\)/);
	assert.match(backendServices, /mime_type LIKE 'image\/%'/);
	assert.match(r2Client, /pub async fn download_file/);
});

test('question bank Word math conversion preserves structured formulas as OMML', () => {
	const mathMl = `<math xmlns="http://www.w3.org/1998/Math/MathML">${convertLatexToMathMl(
		String.raw`\int_0^1 \frac{x^2}{\sqrt{1+x}}\,dx`
	)}</math>`;
	const omml = mml2omml(mathMl);

	assert.match(omml, /^<m:oMath\b/);
	assert.match(omml, /<m:nary>/);
	assert.match(omml, /<m:f>/);
	assert.match(omml, /<m:rad>/);
	assert.doesNotMatch(omml, /<img\b|data:image\//);
});

test('question bank Word math keeps function names upright with compact function spacing', () => {
	const mathMl =
		'<math xmlns="http://www.w3.org/1998/Math/MathML"><mrow>' +
		'<mi fontstyle="normal">sin</mi><mo>&#x2061;</mo><mi>x</mi>' +
		'</mrow></math>';
	const omml = mml2omml(mathMl);

	assert.match(omml, /<m:rPr><m:sty m:val="p"\/><\/m:rPr><m:t xml:space="preserve">sin<\/m:t>/);
	assert.doesNotMatch(omml, /<m:nor\/>/);
	assert.doesNotMatch(omml, /<m:t xml:space="preserve"> <\/m:t>/);
	assert.match(omml, /<m:r><m:t xml:space="preserve">⁡x<\/m:t><\/m:r>/);
});
