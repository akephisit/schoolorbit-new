import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../..');
const route = 'src/routes/(app)/staff/academic/question-bank';

async function source(file) {
	return readFile(path.join(root, file), 'utf8');
}

test('question bank starts independent default regions in its route loader', async () => {
	const loader = await source(`${route}/+page.ts`);
	const page = await source(`${route}/+page.svelte`);
	assert.match(loader, /captureRouteLoad\(\s*getQuestionBankOptions\(\{ requestFetch: fetch \}\)/);
	assert.match(
		loader,
		/captureRouteLoad\(\s*listQuestionBankQuestions\(\{ page: 1, pageSize: 20 \}, \{ requestFetch: fetch \}\)/
	);
	assert.doesNotMatch(page, /onMount\(\(\) => \{\s*void loadInitialData\(\)/);
	assert.match(page, /data\.options/);
	assert.match(page, /data\.questionPage/);
	assert.match(page, /data-testid="question-bank-options-loading"/);
	assert.match(page, /data-testid="question-bank-list-loading"/);
});

test('question bank keeps optional editor and detail off the primary loading path', async () => {
	const page = await source(`${route}/+page.svelte`);
	const api = await source('src/lib/api/questionBank.ts');
	assert.match(page, /import\('#lib\/components\/question-bank\/QuestionContentEditor\.svelte'\)/);
	assert.doesNotMatch(page, /import QuestionContentEditor from/);
	assert.match(page, /const questionsRequest = new LatestRequest\(\)/);
	assert.match(page, /const detailRequest = new LatestRequest\(\)/);
	assert.match(api, /getQuestionBankOptions\(\s*options: ApiRequestOptions = \{\}/);
	assert.match(
		api,
		/getQuestionBankQuestion\(\s*id: QuestionId,\s*options: ApiRequestOptions = \{\}/
	);
});
